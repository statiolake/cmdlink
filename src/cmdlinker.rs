use clap::StructOpt;
use itertools::Itertools;
use std::ffi::OsStr;
use std::fs::write;
use std::path::Path;
use std::process::exit;

#[derive(clap::Parser, Debug)]
struct Args {
    program_path: String,
    alias_name: Option<String>,
    args: Vec<String>,
    #[clap(short, long, required = true)]
    into: String,
}

macro_rules! error {
    ($fmt:literal $(,$args:expr)*) => {{
        eprintln!(concat!("error: ", $fmt) $(, $args)*);
        exit(1);
    }}
}

macro_rules! unwrap {
    ($e:expr) => {
        match $e {
            Ok(value) => value,
            Err(e) => error!("{:?}", e),
        }
    };
}

fn main() {
    let args = Args::parse();

    let into = Path::new(&args.into);
    if !into.exists() || !into.is_dir() {
        error!(
            "directory {} does not exist or not a directory",
            into.display()
        );
    }

    if let Some(alias_name) = &args.alias_name {
        // Single file alias mode
        let exec = match which::which(&args.program_path) {
            Ok(path) => path,
            Err(e) => error!("{} not found: {}", args.program_path, e),
        };
        let mut alias = into.to_path_buf();
        alias.push(&alias_name);
        create_alias(&exec, &alias, args.args);
    } else {
        // Directory file alias mode
        let exec = Path::new(&args.program_path);
        if !exec.exists() || !exec.is_dir() {
            error!("directory {} not found", args.program_path);
        }

        for entry in unwrap!(exec.read_dir()) {
            let entry = unwrap!(entry);
            let exec = entry.path();
            let meta = unwrap!(entry.metadata());
            if !meta.is_file() {
                continue;
            }
            if exec.extension() != Some(OsStr::new("exe")) {
                continue;
            }
            let mut alias = into.to_path_buf();
            alias.push(exec.file_name().unwrap());
            create_alias(&exec, &alias, vec![]);
        }
    }
}

fn create_alias(exec: &Path, alias: &Path, mut args: Vec<String>) {
    if args.is_empty() {
        // default argument
        args = vec!["%*".to_string()];
    }

    let metadata_file_contents = format!(
        r#"return {{
    program_path = [[{program_path}]],
    gen_args = function(args)
        return {{{args}}}
    end,
    gui = false,
    background = false,
}}"#,
        program_path = exec.display(),
        args = args.iter().map(|s| escape(s)).format(", "),
    );

    if write(alias.with_extension("meta"), &metadata_file_contents).is_err() {
        error!("writing metadata failed");
    }

    println!("alias created: {}", alias.display());
    println!("{}", metadata_file_contents);
    println!("run `cmdlink update` to enable this alias")
}

fn escape(arg: &str) -> String {
    if arg == "%*" {
        "unpack(args)".to_string()
    } else if arg.contains('\\') {
        format!("{{{}}}", arg)
    } else {
        format!("\"{}\"", arg)
    }
}
