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
    let mut dialect = rice::DEFAULT_DIALECT;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if parse_options && arg == "--" {
            parse_options = false;
        } else if parse_options && arg == "--compat" {
            dialect = rice::Dialect::QuickBasic;
        } else if parse_options && arg == "--dialect" {
            if i + 1 >= args.len() {
                eprintln!("error: --dialect requires an argument (ansi, qb, or qbasic)");
                process::exit(1);
            }
            i += 1;
            match args[i].to_lowercase().as_str() {
                "ansi" => dialect = rice::Dialect::Ansi,
                "qb" | "qbasic" | "quickbasic" => dialect = rice::Dialect::QuickBasic,
                other => {
                    eprintln!("error: unknown dialect: {other}");
                    process::exit(1);
                }
            }
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
        None => {
            let mut repl = rice::repl::Repl::with_dialect(dialect);
            repl.run();
        }
        Some(filename) => {
            let mut interpreter = rice::interpreter::Interpreter::new();
            interpreter.dialect = dialect;
            interpreter.set_command_line_args(&program_args);
            if let Err(e) = interpreter.run_file(&filename) {
                eprintln!("{e}");
                process::exit(1);
            }
        }
    }
}
