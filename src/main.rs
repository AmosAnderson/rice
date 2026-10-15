use std::env;
use std::process;

fn main() {
    // Run on a thread with a larger stack to support deep recursion.
    // In debug builds, Rust's match arms in exec_stmt/eval_expr create
    // large stack frames (~100KB each), exhausting the default 1MB Windows
    // stack at only ~10 levels of recursion.
    const STACK_SIZE: usize = 8 * 1024 * 1024; // 8 MB
    let builder = std::thread::Builder::new().stack_size(STACK_SIZE);
    let handler = builder.spawn(run).unwrap();
    if handler.join().is_err() {
        process::exit(1);
    }
}

fn run() {
    let args: Vec<String> = env::args().collect();

    let mut source_file: Option<String> = None;
    let mut program_args = Vec::new();
    let mut parse_options = true;
    let mut check_only = false;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if parse_options && arg == "--" {
            parse_options = false;
        } else if parse_options && arg == "--check" {
            check_only = true;
        } else if parse_options && arg.starts_with('-') {
            eprintln!("error: unknown option: {arg}");
            process::exit(1);
        } else if source_file.is_none() {
            source_file = Some(arg.clone());
        } else {
            program_args.push(arg.clone());
        }
        i += 1;
    }

    match source_file {
        None if check_only => {
            eprintln!("error: --check requires a source file");
            process::exit(1);
        }
        None => {
            let mut repl = rice::repl::Repl::new();
            repl.run();
        }
        Some(filename) => {
            let result = if check_only {
                check_file(&filename)
            } else {
                let mut interpreter = rice::interpreter::Interpreter::new();
                interpreter.set_command_line_args(&program_args);
                interpreter.run_file(&filename)
            };
            if let Err(e) = result {
                eprintln!("{e}");
                process::exit(1);
            }
        }
    }
}

fn check_file(filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(filename);
    let canonical =
        std::fs::canonicalize(path).map_err(|e| rice::error::RuntimeError::General {
            msg: format!("Cannot open '{}': {}", path.display(), e),
        })?;
    let source =
        std::fs::read_to_string(&canonical).map_err(|e| rice::error::RuntimeError::General {
            msg: format!("Cannot read '{}': {}", canonical.display(), e),
        })?;
    let tokens = rice::lexer::Lexer::new(&source).tokenize()?;
    rice::parser::Parser::new(tokens).parse_program()?;
    Ok(())
}
