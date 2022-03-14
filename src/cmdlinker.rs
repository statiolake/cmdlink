use itertools::Itertools;
use std::env::args;
use std::fs::write;
use std::os::windows::fs::symlink_file;
use std::path::Path;

fn main() {
    let args = args().collect::<Vec<_>>();
    if args.len() < 3 {
        eprintln!(
            "usage: {} {{program_path}} {{alias_name}} {{args...}}",
            args[0]
        );
    }

    let program_path = match which::which(&args[1]) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("error: {} not found: {}", args[1], e);
            return;
        }
    };
    let alias_name = &args[2];
    let args = &args[3..];

    let cmdlink = Path::new("cmdlink.exe");
    if !cmdlink.exists() {
        eprintln!("error: cmdlink.exe is not found");
    }

    let metadata_file_contents = vec![
        format!("path = {}", program_path.display()),
        format!("args = [{}]", args.iter().map(|s| escape(s)).format(", ")),
    ]
    .join("\n");

    let alias = Path::new(alias_name).with_extension("exe");

    if symlink_file(cmdlink, &alias).is_err() {
        eprintln!("error: creating symlink failed; make sure you have a permission");
        return;
    }

    if write(alias.with_extension("meta"), &metadata_file_contents).is_err() {
        eprintln!("error: writing metadata failed");
        return;
    }

    println!("alias created: {}", alias_name);
    println!("{}", metadata_file_contents);
}

fn escape(arg: &str) -> String {
    format!("\"{}\"", arg)
}
