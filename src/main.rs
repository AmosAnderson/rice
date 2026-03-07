use std::env;
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
            let mut interpreter = rice::interpreter::Interpreter::new();
            if let Err(e) = interpreter.run_file(filename) {
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
