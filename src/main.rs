use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        1 => {
            let mut repl = rice::repl::Repl::new();
            repl.run();
        }
        2 => {
            let filename = &args[1];
            let source = match fs::read_to_string(filename) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error reading {filename}: {e}");
                    process::exit(1);
                }
            };
            let mut interpreter = rice::interpreter::Interpreter::new();
            if let Err(e) = interpreter.run_source(&source) {
                eprintln!("{e}");
                process::exit(1);
            }
        }
        _ => {
            eprintln!("Usage: rice [filename.bas]");
            process::exit(1);
        }
    }
}
