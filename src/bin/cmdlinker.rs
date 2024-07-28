use std::ffi::OsStr;
use std::process;
use std::{env, fs};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::Result;
use anyhow::{anyhow, ensure, Context};
use clap::Parser;

use cmdlink::{Metadata, MetadataWriter};

fn main() {
    let args = Args::parse();
    if let Err(e) = args.run() {
        let mut e: &dyn std::error::Error = &*e;
        let mut label = "error";
        loop {
            eprintln!("{label}: {}", e);
            e = match e.source() {
                Some(source) => source,
                None => break,
            };
            label = "due to";
        }

        process::exit(1);
    }
}

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
        let exe = find_cmdlink_executables()?;
        let alias = Alias::try_from_stem_path(root_path.join(&self.alias))?;
        create_metadata(&exe, &prog_path, &alias, &self.args)?;

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
        let exe = find_cmdlink_executables()?;
        create_metadata_in_dir(&exe, prog_dir_path, root_path)?;

        Ok(())
    }
}

#[derive(Parser, Debug)]
struct UpdateArgs {
    #[clap(short, long, default_value = ".")]
    root_path: String,
}

impl UpdateArgs {
    fn run(&self) -> Result<()> {
        let root_path = check_dir_exists(&self.root_path)?;

        let exe = find_cmdlink_executables()?;
        println!("cmdlink.exe found at:");
        println!("  CUI: {}", exe.cui.display());
        println!("  GUI: {}", exe.gui.display());

        for alias in find_all_aliases_in_dir(root_path)? {
            if let Err(err) = update_alias(&exe, &alias?) {
                eprintln!("error: {}; skipping", err);
            }
        }

        Ok(())
    }
}

// #[derive(Debug)]
// pub struct Config {
//     lua: Lua,
// }
//
// impl Config {
//     pub fn load(root_path: &Path) -> Result<Config> {
//         let config_path = root_path.join("config.lua");
//         let lua = Lua::new();
//         lua.context(|ctx| -> Result<()> {
//             let source = read_to_string(&config_path)?;
//             let table: Table = ctx.load(&source).eval()?;
//             ctx.globals().set("__config", table)?;
//             Ok(())
//         })
//         .map_err(|err| {
//             anyhow!(
//                 "failed to load config file at '{}': {}",
//                 config_path.display(),
//                 err
//             )
//         })?;
//
//         Ok(Config { lua })
//     }
//
//     pub fn update_paths(&self) -> Result<Vec<PathBuf>> {
//         self.lua.context(|ctx| {
//             let config: Table = ctx.globals().get("__config")?;
//             let paths: Vec<String> = config.get("paths")?;
//             Ok(paths.into_iter().map(PathBuf::from).collect())
//         })
//     }
// }

fn check_dir_exists<P: AsRef<Path>>(path: &P) -> Result<&Path> {
    let path = path.as_ref();
    ensure!(
        path.exists() && path.is_dir(),
        "directory '{}' does not exist or not a directory",
        path.display()
    );

    Ok(path)
}

fn create_metadata<S: AsRef<str>>(
    exe: &CmdlinkExecutable,
    prog_path: &Path,
    alias: &Alias,
    args: &[S],
) -> Result<()> {
    let mut writer = MetadataWriter::new(prog_path.to_owned());
    if !args.is_empty() {
        let args = args.iter().map(|arg| arg.as_ref().to_string()).collect();
        writer.args(args);
    }

    writer.write(&alias.meta_path)?;
    update_alias(exe, alias)?;
    println!("{}", prog_path.display());

    Ok(())
}

fn create_metadata_in_dir(
    exe: &CmdlinkExecutable,
    prog_dir_path: &Path,
    root_path: &Path,
) -> Result<()> {
    for entry in prog_dir_path.read_dir()? {
        let entry = entry?;
        let prog_path = entry.path();
        if !entry.metadata()?.is_file() {
            continue;
        }

        let ext = match prog_path.extension() {
            Some(ext) => ext,
            None => continue,
        };

        let alias_name = prog_path.file_name().unwrap();
        let alias = Alias::try_from_stem_path(root_path.join(alias_name))?;
        match ext.to_str() {
            Some("exe") => create_metadata(exe, &prog_path, &alias, &Vec::<&str>::new())?,
            Some("cmd" | "bat") => todo!(),
            _ => continue,
        }
    }

    Ok(())
}

fn update_alias(exe: &CmdlinkExecutable, alias: &Alias) -> Result<()> {
    let metadata = Metadata::parse(alias.meta_path.clone())
        .with_context(|| format!("failed to parse metadata for {}", alias.meta_path.display()))?;

    let is_gui = metadata.gui().context("failed to read 'gui'")?;
    let exe = if is_gui { &exe.gui } else { &exe.cui };

    fs::copy(exe, &alias.alias_path).with_context(|| {
        format!(
            "failed to copy {} to {}",
            exe.display(),
            alias.alias_path.display()
        )
    })?;

    println!(
        "updated {} (by {})",
        alias.alias_path.display(),
        exe.display()
    );

    Ok(())
}

#[derive(Debug, Clone)]
struct CmdlinkExecutable {
    cui: PathBuf,
    gui: PathBuf,
}

fn find_cmdlink_executables() -> Result<CmdlinkExecutable> {
    let cmdlinker_exe = env::current_exe()?;
    let os_ext = cmdlinker_exe.extension().unwrap_or_else(|| OsStr::new(""));

    let cui_exe = cmdlinker_exe
        .with_file_name("cmdlink")
        .with_extension(os_ext);
    let mut gui_exe = cmdlinker_exe
        .with_file_name("gcmdlink")
        .with_extension(os_ext);

    if !gui_exe.exists() {
        println!("GUI cmdlink does not found; fallbacking to CUI cmdlink");
        gui_exe = cui_exe.clone();
    }

    Ok(CmdlinkExecutable {
        cui: cui_exe,
        gui: gui_exe,
    })
}

#[derive(Debug, Clone)]
struct Alias {
    alias_path: PathBuf,
    meta_path: PathBuf,
}

impl Alias {
    pub fn try_from_alias_path(alias_path: PathBuf) -> Result<Self> {
        if alias_path.extension().unwrap_or(OsStr::new("")) != get_os_ext()? {
            return Err(anyhow!("invalid extension"));
        }

        Self::try_from_stem_path(alias_path.with_extension(""))
    }

    pub fn try_from_meta_path(meta_path: PathBuf) -> Result<Self> {
        if meta_path.extension() != Some(OsStr::new("meta")) {
            return Err(anyhow!("invalid extension"));
        }

        Self::try_from_stem_path(meta_path.with_extension(""))
    }

    pub fn try_from_stem_path(stem_path: PathBuf) -> Result<Self> {
        let alias_path = stem_path.with_extension(get_os_ext()?);
        let meta_path = stem_path.with_extension("meta");

        Ok(Self {
            alias_path,
            meta_path,
        })
    }
}

fn get_os_ext() -> Result<OsString> {
    Ok(env::current_exe()?
        .extension()
        .unwrap_or_else(|| OsStr::new(""))
        .to_owned())
}

fn find_all_aliases_in_dir(root_path: &Path) -> Result<Vec<Result<Alias>>> {
    Ok(root_path
        .read_dir()?
        .map(|entry| -> Result<_> {
            let entry = entry?;
            Ok(Alias::try_from_meta_path(entry.path()).ok())
        })
        .filter_map(Result::transpose)
        .collect())
}
