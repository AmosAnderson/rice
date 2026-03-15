/// AST → RiceIR lowering pass.
///
/// Walks the AST and produces a linear sequence of IR instructions.
/// Phase 1: supports PRINT with string/numeric literals and END.

use crate::ast::*;
use crate::compiler::ir::*;

/// Lowering context — tracks temp and label allocation
pub struct Lowerer {
    next_temp: TempId,
    next_label: IrLabel,
    instructions: Vec<Instruction>,
}

impl Lowerer {
    pub fn new() -> Self {
        Self {
            next_temp: 0,
            next_label: 0,
            instructions: Vec::new(),
        }
    }

    fn alloc_temp(&mut self) -> TempId {
        let t = self.next_temp;
        self.next_temp += 1;
        t
    }

    #[allow(dead_code)]
    fn alloc_label(&mut self) -> IrLabel {
        let l = self.next_label;
        self.next_label += 1;
        l
    }

    fn emit(&mut self, inst: Instruction) {
        self.instructions.push(inst);
    }

    /// Lower an entire program to IR
    pub fn lower_program(mut self, program: &Program) -> Result<IrProgram, String> {
        for labeled_stmt in &program.statements {
            self.lower_stmt(&labeled_stmt.stmt)?;
        }
        // Ensure program ends
        self.emit(Instruction::End);

        Ok(IrProgram {
            main: IrFunction {
                name: "main".to_string(),
                instructions: self.instructions,
            },
        })
    }

    fn lower_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Print(print_stmt) => self.lower_print(print_stmt),
            Stmt::End | Stmt::System => {
                self.emit(Instruction::End);
                Ok(())
            }
            Stmt::Rem => Ok(()), // skip comments
            _ => Err(format!("unsupported statement: {:?}", std::mem::discriminant(stmt))),
        }
    }

    fn lower_print(&mut self, print_stmt: &PrintStmt) -> Result<(), String> {
        if print_stmt.format.is_some() {
            return Err("PRINT USING not yet supported in compiler".to_string());
        }

        for item in &print_stmt.items {
            match item {
                PrintItem::Expr(expr) => {
                    let temp = self.lower_expr(expr)?;
                    self.emit(Instruction::PrintValue(temp, 0));
                }
                PrintItem::Comma => {
                    self.emit(Instruction::PrintComma);
                }
                PrintItem::Tab(_) | PrintItem::Spc(_) => {
                    return Err("TAB/SPC not yet supported in compiler".to_string());
                }
            }
        }

        match print_stmt.trailing {
            PrintSep::Newline => {
                self.emit(Instruction::PrintNewline);
            }
            PrintSep::Semicolon => {}
            PrintSep::Comma => {
                self.emit(Instruction::PrintComma);
            }
        }

        Ok(())
    }

    fn lower_expr(&mut self, expr: &Expr) -> Result<TempId, String> {
        match expr {
            Expr::StringLit(s) => {
                let t = self.alloc_temp();
                self.emit(Instruction::LoadConst(t, Constant::Str(s.clone())));
                Ok(t)
            }
            Expr::IntegerLit(n) => {
                let t = self.alloc_temp();
                self.emit(Instruction::LoadConst(t, Constant::Integer(*n)));
                Ok(t)
            }
            Expr::DoubleLit(n) => {
                let t = self.alloc_temp();
                self.emit(Instruction::LoadConst(t, Constant::Double(*n)));
                Ok(t)
            }
            Expr::Paren(inner) => self.lower_expr(inner),
            Expr::BinaryOp { left, op, right } => {
                let l = self.lower_expr(left)?;
                let r = self.lower_expr(right)?;
                let result = self.alloc_temp();
                self.emit(Instruction::BinOp(result, *op, l, r));
                Ok(result)
            }
            Expr::UnaryOp { op, operand } => {
                let o = self.lower_expr(operand)?;
                let result = self.alloc_temp();
                self.emit(Instruction::UnaryOp(result, *op, o));
                Ok(result)
            }
            _ => Err(format!("unsupported expression: {:?}", std::mem::discriminant(expr))),
        }
    }
}
