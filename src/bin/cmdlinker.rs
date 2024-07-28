use anyhow::{anyhow, ensure};
use anyhow::{Ok, Result};
use clap::Parser;
use cmdlink::MetadataWriter;
use itertools::Itertools;
use rlua::{Lua, Table};
use std::fs::{read_to_string, write};
use std::path::{Path, PathBuf};
use std::process::exit;

#[derive(Parser, Debug)]
enum Args {
    Create(CreateArgs),
    CreateAll(CreateAllArgs),
    Update(UpdateArgs),
}

impl Args {
    fn run(&self) -> Result<()> {
        match self {
            Args::Create(args) => args.run(),
            Args::CreateAll(args) => args.run(),
            Args::Update(args) => args.run(),
        }
    }
}

#[derive(Parser, Debug)]
struct CreateArgs {
    #[clap(short, long, required = true)]
    root_path: String,
    prog_path: String,
    alias: String,
    args: Vec<String>,
}

impl CreateArgs {
    fn run(&self) -> Result<()> {
        let root_path = check_dir_exists(&self.root_path)?;
        let prog_path = which::which(&self.prog_path)?;
        let alias_path = root_path.join(&self.alias);
        create_alias(&prog_path, &alias_path, &self.args)?;

        Ok(())
    }
}

#[derive(Parser, Debug)]
struct CreateAllArgs {
    #[clap(short, long, required = true)]
    root_path: String,
    prog_dir_path: String,
}

impl CreateAllArgs {
    fn run(&self) -> Result<()> {
        let root_path = check_dir_exists(&self.root_path)?;
        let prog_dir_path = check_dir_exists(&self.prog_dir_path)?;
        create_alias_in_dir(prog_dir_path, root_path)?;
        Ok(())
    }
}

#[derive(Parser, Debug)]
struct UpdateArgs {
    #[clap(short, long, required = true)]
    root_path: String,
}

impl UpdateArgs {
    fn run(&self) -> Result<()> {
        todo!()
    }
}

#[derive(Debug)]
pub struct Config {
    lua: Lua,
}

impl Config {
    pub fn load(root_path: &Path) -> Result<Config> {
        let config_path = root_path.join("config.lua");
        let lua = Lua::new();
        lua.context(|ctx| -> Result<()> {
            let source = read_to_string(&config_path)?;
            let table: Table = ctx.load(&source).eval()?;
            ctx.globals().set("__config", table)?;
            Ok(())
        })
        .map_err(|err| {
            anyhow!(
                "failed to load config file at '{}': {}",
                config_path.display(),
                err
            )
        })?;

        Ok(Config { lua })
    }

    pub fn update_paths(&self) -> Result<Vec<PathBuf>> {
        self.lua.context(|ctx| {
            let config: Table = ctx.globals().get("__config")?;
            let paths: Vec<String> = config.get("paths")?;
            Ok(paths.into_iter().map(PathBuf::from).collect())
        })
    }
}

fn check_dir_exists<P: AsRef<Path>>(path: &P) -> Result<&Path> {
    let path = path.as_ref();
    ensure!(
        path.exists() && path.is_dir(),
        "directory '{}' does not exist or not a directory",
        path.display()
    );

    Ok(path)
}

fn main() {
    let args = Args::parse();
    if let Err(e) = args.run() {
        eprintln!("error: {}", e);
        exit(1);
    }
}

fn create_alias<S: AsRef<str>>(prog_path: &Path, alias_path: &Path, args: &[S]) -> Result<()> {
    let mut writer = MetadataWriter::new(prog_path.to_owned());
    if !args.is_empty() {
        let args = args.iter().map(|arg| arg.as_ref().to_string()).collect();
        writer.args(args);
    }
    let metadata_path = alias_path.with_extension("meta");
    writer.write(&metadata_path)?;
    println!("{}", alias_path.display());

    Ok(())
}

fn create_alias_in_dir(prog_dir_path: &Path, root_path: &Path) -> Result<()> {
    for entry in prog_dir_path.read_dir()? {
        let entry = entry?;
        let prog_path = entry.path();
        let meta = entry.metadata()?;

        if !meta.is_file() {
            continue;
        }

        let ext = match prog_path.extension() {
            Some(ext) => ext,
            None => continue,
        };

        let alias_path = root_path.join(prog_path.file_name().unwrap());
        match ext.to_str() {
            Some("exe") => create_alias(&prog_path, &alias_path, &Vec::<&str>::new())?,
            Some("cmd" | "bat") => todo!(),
            _ => continue,
        }
    }

    Ok(())
}
