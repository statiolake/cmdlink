use std::env::{args, current_exe};
use std::ffi::OsStr;
use std::fs::{copy, read_dir};
use std::path::Path;
use std::process::exit;

use cmdlink::{Metadata, Result};

fn run_maintainance_mode(args: Vec<String>) -> Result<()> {
    if args.get(1).map(|x| &**x) == Some("update") {
        update_aliases()?;
    }

    Ok(())
}

fn update_aliases() -> Result<()> {
    let exe = current_exe()?;
    let gexe = {
        let mut gexe = exe.with_file_name("gcmdlink.exe");
        if !gexe.exists() {
            println!("GUI cmdlink does not found; fallbacking to CUI cmdlink");
            gexe = exe.clone();
        }
        gexe
    };

    println!("cmdlink.exe found at:");
    println!("  CUI: {}", exe.display());
    println!("  GUI: {}", gexe.display());

    for entry in read_dir(".")? {
        let entry = entry?;
        if entry.path().extension() != Some(OsStr::new("meta")) {
            continue;
        }
        let alias = entry.path().with_extension("exe");

        let metadata = Metadata::parse(entry.path())?;
        let exe = if metadata.gui()? { &gexe } else { &exe };

        copy(exe, &alias)?;
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
