//! The `tdoc` command: reads its arguments, then hands the terminal to the editor.

use std::process::ExitCode;

const USAGE: &str = "Usage: tdoc [file]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        [flag] if flag == "--version" || flag == "-V" => {
            println!("tdoc {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [] => ExitCode::SUCCESS,
        [path] => match std::fs::read(path) {
            Ok(_) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("tdoc: cannot open {path}: {error}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
