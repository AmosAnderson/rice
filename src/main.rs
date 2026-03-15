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

    let mut compile = false;
    let mut emit_ir = false;
    let mut output: Option<String> = None;
    let mut source_file: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--compile" => compile = true,
            "--emit-ir" => emit_ir = true,
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: -o requires an argument");
                    process::exit(1);
                }
                output = Some(args[i].clone());
            }
            arg if arg.starts_with('-') => {
                eprintln!("error: unknown option: {arg}");
                process::exit(1);
            }
            _ => {
                source_file = Some(args[i].clone());
            }
        }
        i += 1;
    }

    if compile || emit_ir {
        let source = match &source_file {
            Some(f) => f.clone(),
            None => {
                eprintln!("error: --compile/--emit-ir requires a source file");
                process::exit(1);
            }
        };

        if emit_ir {
            let src = std::fs::read_to_string(&source).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                process::exit(1);
            });
            match rice::compiler::emit_ir(&src) {
                Ok(ir) => print!("{ir}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            return;
        }

        // Determine output path
        let out = output.unwrap_or_else(|| {
            let p = std::path::Path::new(&source);
            let stem = p.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if cfg!(target_os = "windows") {
                format!("{stem}.exe")
            } else {
                stem
            }
        });

        match rice::compiler::compile_file(&source, &out) {
            Ok(()) => {
                eprintln!("compiled: {source} -> {out}");
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
        return;
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
