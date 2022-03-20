use itertools::Itertools;
use std::env::args;
use std::fs::write;
use std::path::Path;
use std::process::exit;

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
            exit(1);
        }
    };
    let alias_name = &args[2];
    let args = &args[3..];

    let metadata_file_contents = format!(
        r#"return {{
    program_path = [[{program_path}]],
    gen_args = function(args)
        return {{{args}}}
    end,
}}"#,
        program_path = program_path.display(),
        args = args.iter().map(|s| escape(s)).format(", "),
    );

    let alias = Path::new(alias_name).with_extension("exe");

    if write(alias.with_extension("meta"), &metadata_file_contents).is_err() {
        eprintln!("error: writing metadata failed");
        exit(1);
    }

    println!("alias created: {}", alias_name);
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
