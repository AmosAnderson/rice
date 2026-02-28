use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::parser::Parser;

pub struct Repl {
    interpreter: Interpreter,
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

impl Repl {
    pub fn new() -> Self {
        Self {
            interpreter: Interpreter::new(),
        }
    }

    pub fn run(&mut self) {
        println!("Rice BASIC v0.1.0");
        println!("Type END or press Ctrl+D to exit.");
        println!();

        let mut editor = DefaultEditor::new().expect("failed to create editor");
        let history_file = dirs_history_path();
        let _ = editor.load_history(&history_file);

        loop {
            match editor.readline("Ok\n") {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let _ = editor.add_history_entry(trimmed);

                    match self.execute_line(trimmed) {
                        Ok(true) => break, // END was executed
                        Ok(false) => {}
                        Err(e) => eprintln!("{e}"),
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    break;
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    break;
                }
            }
        }

        let _ = editor.save_history(&history_file);
    }

    fn execute_line(&mut self, line: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let tokens = Lexer::new(line).tokenize()?;
        let program = Parser::new(tokens).parse_program()?;
        // Check if any statement is END
        let has_end = program
            .statements
            .iter()
            .any(|s| matches!(s.stmt, crate::ast::Stmt::End));
        self.interpreter.run_program(&program)?;
        Ok(has_end)
    }
}

fn dirs_history_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    format!("{home}/.rice_history")
}
