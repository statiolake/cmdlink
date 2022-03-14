use std::process::exit;

fn main() {
    match cmdlink::run_for_current_exe() {
        Ok(code) => {
            exit(code);
        }
        Err(e) => {
            eprintln!("error: {}", e);
            exit(1);
        }
    }
}
