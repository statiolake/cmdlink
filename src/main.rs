use cmdlink::{Metadata, Result};
use std::env::{args, current_exe};
use std::ffi::OsStr;
use std::fs::{copy, read_dir};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::exit;

fn run_maintainance_mode(args: Vec<String>) -> Result<()> {
    if args.get(1).map(|x| x.deref()) == Some("update") {
        update_aliases()?;
    }

    Ok(())
}

fn get_exe_files() -> Result<(PathBuf, PathBuf)> {
    let exe = current_exe()?;
    let mut gexe = exe.with_file_name("gcmdlink.exe");
    if !gexe.exists() {
        println!("GUI cmdlink does not found; fallbacking to CUI cmdlink");
        gexe = exe.clone();
    }

    Ok((exe, gexe))
}

fn update_aliases() -> Result<()> {
    let (exe, gexe) = get_exe_files()?;
    println!("cmdlink.exe found at:");
    println!("  CUI: {}", exe.display());
    println!("  GUI: {}", gexe.display());

    for entry in read_dir(".")? {
        macro_rules! gentle_unwrap {
            ($e:expr, $msg:literal $(, $args:expr)* $(,)?) => {
                match $e {
                    Ok(value) => value,
                    Err(err) => {
                        eprintln!($msg, $($args,)* err = err);
                        continue;
                    }
                }
            }
        }

        let entry = gentle_unwrap!(entry, "failed to read some entry: {err}, skipping");
        if entry.path().extension() != Some(OsStr::new("meta")) {
            // not a meta file, skipping.
            continue;
        }

        let alias = entry.path().with_extension("exe");
        let metadata = gentle_unwrap!(
            Metadata::parse(entry.path()),
            "failed to parse metadata for {}: {err}, skipping",
            alias.display(),
        );
        let is_gui = gentle_unwrap!(metadata.gui(), "failed to read 'gui': {err}, skipping");
        let exe = if is_gui { &gexe } else { &exe };

        // copy the binary
        gentle_unwrap!(
            copy(exe, &alias),
            "failed to copy {} to {}: {err}, skipping",
            exe.display(),
            alias.display(),
        );

        println!("updated {} (by {})", alias.display(), exe.display());
    }

    Ok(())
}

pub fn run_alias_mode() -> Result<i32> {
    let exe = current_exe()?;
    if !exe.exists() {
        panic!("internal error: link execuable does not exist")
    }

    let metadata = Metadata::load(&exe)?;
    cmdlink::run(&metadata)
}

fn main() {
    let args: Vec<String> = args().collect();

    let res = if Path::new(&args[0]).file_stem() == Some(OsStr::new("cmdlink")) {
        run_maintainance_mode(args).map(|()| 0).map_err(Into::into)
    } else {
        run_alias_mode()
    };

    match res {
        Ok(code) => {
            exit(code);
        }
        Err(e) => {
            eprintln!("error: {}", e);
            exit(1);
        }
    }
}
