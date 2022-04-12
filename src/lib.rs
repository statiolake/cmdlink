use rlua::{Context, Function, Lua, Table};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, io};

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

fn register_builtins(lua: &Lua) -> rlua::Result<()> {
    lua.context(|ctx| -> rlua::Result<()> { ctx.load(include_str!("globals.lua")).exec() })?;

    Ok(())
}

fn config(ctx: Context) -> rlua::Result<Table> {
    ctx.globals().get("__config")
}

pub fn run(metadata: &Metadata) -> Result<i32> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let prog_path = &metadata.prog_path()?;

    let mut cmd = Command::new(prog_path);
    cmd.args(metadata.gen_args(args)?);
    if !metadata.gui()? {
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
    }
    cmd.envs(metadata.get_envvars()?);

    let mut child = cmd.spawn()?;

    if metadata.background()? {
        Ok(0)
    } else {
        let res = child.wait()?;
        Ok(res.code().unwrap_or(1))
    }
}
