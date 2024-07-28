use cmdlink::{Metadata, Result};
use std::env::current_exe;
use std::process::exit;

pub fn run_alias_mode() -> Result<i32> {
    let exe = current_exe()?;
    if !exe.exists() {
        panic!("internal error: link execuable does not exist")
    }

    let metadata = Metadata::load(&exe)?;
    cmdlink::run(&metadata)
}

fn main() {
    let res = run_alias_mode();
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
