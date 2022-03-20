use std::env::{args, current_exe};
use std::ffi::OsStr;
use std::fs::{copy, read_dir};
use std::io;
use std::path::Path;
use std::process::exit;

pub fn run_alias_mode() -> cmdlink::Result<i32> {
    let exe = current_exe()?;
    if !exe.exists() {
        panic!("internal error: link execuable does not exist")
    }

    let metadata = cmdlink::Metadata::load(&exe)?;
    cmdlink::run(&metadata)
}

fn update_aliases() -> io::Result<()> {
    let exe = current_exe()?;
    println!("updating from {}...", exe.display());

    for entry in read_dir(".")? {
        let entry = entry?;
        if entry.path().extension() != Some(OsStr::new("meta")) {
            continue;
        }
        let alias = entry.path().with_extension("exe");
        copy(&exe, &alias)?;
        println!("updated {}", alias.display());
    }

    Ok(())
}

fn run_maintainance_mode(args: Vec<String>) -> io::Result<()> {
    if args.get(1).map(|x| &**x) == Some("update") {
        update_aliases()?;
    }

    Ok(())
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
