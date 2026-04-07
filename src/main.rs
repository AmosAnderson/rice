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

    for arg in &args[1..] {
        if arg.starts_with('-') {
            eprintln!("error: unknown option: {arg}");
            process::exit(1);
        }
        if source_file.is_some() {
            eprintln!("error: unexpected argument: {arg}");
            process::exit(1);
        }
        source_file = Some(arg.clone());
    }

    match source_file {
        None => {
            let mut repl = rice::repl::Repl::new();
            repl.run();
        }
        Some(filename) => {
            let mut interpreter = rice::interpreter::Interpreter::new();
            if let Err(e) = interpreter.run_file(&filename) {
                eprintln!("{e}");
                process::exit(1);
            }
        }
    }
}
