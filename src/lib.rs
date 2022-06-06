use itertools::Itertools as _;
use rlua::{Context, Function, Lua, Table};
use std::fs::{read_to_string, write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{collections::HashMap, env::current_exe};
use std::{env, io};

const BACKGROUND_CHILD_MARKER: &str = "____MARKER__RUN_IN_BACKGROUND_a1a11601____";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {}", .0)]
    IOError(#[from] io::Error),
    #[error("metadata not found for {}", path.display())]
    MetadataNotFound { path: PathBuf },
    #[error("Lua error: {}", .0)]
    LuaError(#[from] rlua::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub struct Metadata {
    lua: Lua,
}

impl Metadata {
    pub fn load(exe: &Path) -> Result<Metadata> {
        let metadata_path = exe.with_extension("meta");
        if !metadata_path.exists() {
            return Err(Error::MetadataNotFound {
                path: metadata_path,
            });
        }

        Metadata::parse(metadata_path)
    }

    pub fn parse(metadata_path: PathBuf) -> Result<Metadata> {
        let lua = Lua::new();
        register_builtins(&lua)?;
        lua.context(|ctx| -> Result<()> {
            let source = read_to_string(&metadata_path).map_err(|_| Error::MetadataNotFound {
                path: metadata_path.clone(),
            })?;
            let table: Table = ctx.load(&source).eval()?;
            ctx.globals().set("__config", table)?;
            Ok(())
        })?;
        Ok(Metadata { lua })
    }

    pub fn prog_path(&self) -> Result<PathBuf> {
        self.lua.context(|ctx| {
            let config = config(ctx)?;
            let prog_path: String = config.get("prog_path")?;
            Ok(PathBuf::from(prog_path))
        })
    }

    pub fn gen_args(&self, args: Vec<String>) -> Result<Vec<String>> {
        self.lua.context(|ctx| {
            let config = config(ctx)?;
            let gen_args: Function = config.get("gen_args")?;
            let args = gen_args.call(args)?;
            Ok(args)
        })
    }

    pub fn restart_args(&self, status: i32) -> Result<Option<Vec<String>>> {
        self.lua.context(|ctx| {
            let config = config(ctx)?;
            let restart_args: Function = match config.get("restart_args") {
                Ok(func) => func,
                Err(_) => return Ok(None),
            };
            let next_args = restart_args.call(status)?;
            Ok(next_args)
        })
    }

    pub fn get_envvars(&self) -> Result<HashMap<String, String>> {
        self.lua.context(|ctx| -> Result<HashMap<String, String>> {
            let table = ctx.create_table()?;
            for (k, v) in env::vars() {
                table.set(k, v)?;
            }

            // add program's directory to path
            let path = self.prog_path()?;
            if let Some(path) = path.parent() {
                let add_to_variable: Function = ctx.globals().get("add_to_variable")?;
                add_to_variable.call((table.clone(), "PATH", path.display().to_string()))?;
            }

            // modify envvars by user-defined key
            let config = config(ctx)?;
            if config.contains_key("modify_envvars")? {
                let modify_envvars: Function = config.get("modify_envvars")?;
                modify_envvars.call(table.clone())?;
            }

            let mut envvars = HashMap::new();
            for pair in table.pairs() {
                let (k, v) = pair?;
                envvars.insert(k, v);
            }

            Ok(envvars)
        })
    }

    pub fn gui(&self) -> Result<bool> {
        self.lua.context(|ctx| {
            let config = config(ctx)?;
            Ok(config.get("gui")?)
        })
    }

    pub fn background(&self) -> Result<bool> {
        self.lua.context(|ctx| {
            let config = config(ctx)?;
            Ok(config.get("background")?)
        })
    }
}

#[derive(Debug)]
pub struct MetadataWriter {
    prog_path: PathBuf,
    args: Vec<String>,
    gui: bool,
    background: bool,
}

impl MetadataWriter {
    pub fn new(prog_path: PathBuf) -> Self {
        Self {
            prog_path,
            args: vec!["%*".to_string()],
            gui: false,
            background: false,
        }
    }

    pub fn args(&mut self, args: Vec<String>) -> &mut Self {
        self.args = args;
        self
    }

    // get_envvars() are not supported

    pub fn gui(&mut self, gui: bool) -> &mut Self {
        self.gui = gui;
        self
    }

    pub fn background(&mut self, background: bool) -> &mut Self {
        self.background = background;
        self
    }

    pub fn write(self, path: &Path) -> io::Result<()> {
        fn escape_arg(arg: &str) -> String {
            if arg == "%*" {
                "unpack(args)".to_string()
            } else if arg.contains('\\') {
                format!("[[{}]]", arg)
            } else {
                format!("\"{}\"", arg)
            }
        }

        let args = self.args.iter().map(|a| escape_arg(a));

        let contents = indoc::formatdoc! {
            r#"
                return {{
                    prog_path = [[{prog_path}]],
                    gen_args = function(args)
                        return {{{args}}}
                    end,
                    gui = {gui},
                    background = {background},
                }}
            "#,
            prog_path = self.prog_path.display(),
            args = args.format(", "),
            gui = self.gui,
            background = self.background,
        };

        write(path, contents)
    }
}

fn register_builtins(lua: &Lua) -> rlua::Result<()> {
    lua.context(|ctx| -> rlua::Result<()> { ctx.load(include_str!("globals.lua")).exec() })?;

    Ok(())
}

fn config(ctx: Context) -> rlua::Result<Table> {
    ctx.globals().get("__config")
}

pub fn run(metadata: &Metadata) -> Result<i32> {
    let mut args = env::args().skip(1).peekable();
    let mut is_child = false;
    if args.peek().map(|s| &**s) == Some(BACKGROUND_CHILD_MARKER) {
        args.next();
        is_child = true;
    }
    let args = args.collect_vec();

    // If this specified as a background process, run itself as a detached process and then run the
    // actual process. This is needed to support restart_args in background command.
    if metadata.background()? && !is_child {
        Command::new(current_exe()?)
            .arg(BACKGROUND_CHILD_MARKER)
            .args(args)
            .spawn()?;
        return Ok(0);
    }

    let prog_path = &metadata.prog_path()?;
    let mut args = metadata.gen_args(args)?;
    loop {
        let mut cmd = Command::new(prog_path);
        cmd.args(args);
        if !metadata.gui()? {
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }
        cmd.envs(metadata.get_envvars()?);

        let res = cmd.spawn()?.wait()?;
        let status = res.code().unwrap_or(1);
        args = match metadata.restart_args(status)? {
            Some(args) => args,
            None => return Ok(status),
        };
    }
}
