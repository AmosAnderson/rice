use crate::ast::*;
use crate::error::ParseError;
use crate::token::{SpannedToken, Token, TypeSuffix};

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();
        self.skip_newlines();
        while !self.at_end() {
            let stmt = self.parse_labeled_stmt()?;
            statements.push(stmt);
            self.skip_newlines();
        }
        Ok(Program { statements })
    }

    fn parse_labeled_stmt(&mut self) -> Result<LabeledStmt, ParseError> {
        let line = self.current_line();
        let mut label = None;

        // Check for line number
        if let Token::LineNumber(n) = self.peek() {
            label = Some(Label::Number(*n));
            self.advance();
        }

        // Check for named label (identifier followed by colon, not part of a statement)
        if label.is_none()
            && let Token::Identifier { name, suffix: None } = self.peek()
        {
            let name = name.clone();
            if self.peek_at(1) == Some(&Token::Colon) {
                label = Some(Label::Name(name));
                self.advance(); // consume identifier
                self.advance(); // consume colon
            }
        }

        // If we hit a newline or EOF after a label, return a Rem as placeholder
        if matches!(self.peek(), Token::Newline | Token::Eof)
            && label.is_some()
        {
            self.skip_newlines();
            return Ok(LabeledStmt {
                label,
                stmt: Stmt::Rem,
                line,
            });
        }

        let stmt = self.parse_statement()?;

        // Consume statement terminator (newline, colon, or EOF)
        if matches!(self.peek(), Token::Newline) {
            self.advance();
        }

        Ok(LabeledStmt { label, stmt, line })
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        match self.peek().clone() {
            Token::KwPrint => self.parse_print(),
            Token::KwLPrint => self.parse_print(), // treat LPRINT same as PRINT for now
            Token::KwLet => {
                self.advance();
                self.parse_assignment()
            }
            Token::KwDim => self.parse_dim(),
            Token::KwConst => self.parse_const(),
            Token::KwInput => self.parse_input(),
            Token::KwLineInput => self.parse_line_input(),
            Token::KwIf => self.parse_if(),
            Token::KwFor => self.parse_for(),
            Token::KwWhile => self.parse_while(),
            Token::KwDo => self.parse_do(),
            Token::KwSelect => self.parse_select(),
            Token::KwGoto => {
                self.advance();
                let label = self.parse_label()?;
                Ok(Stmt::Goto(label))
            }
            Token::KwGosub => {
                self.advance();
                let label = self.parse_label()?;
                Ok(Stmt::Gosub(label))
            }
            Token::KwReturn => {
                self.advance();
                Ok(Stmt::Return)
            }
            Token::KwExit => self.parse_exit(),
            Token::KwEnd => {
                self.advance();
                Ok(Stmt::End)
            }
            Token::KwSystem => {
                self.advance();
                Ok(Stmt::System)
            }
            Token::KwStop => {
                self.advance();
                Ok(Stmt::Stop)
            }
            Token::KwSub => self.parse_sub_def(),
            Token::KwFunction => self.parse_function_def(),
            Token::KwCall => self.parse_call(),
            Token::KwDeclare => self.parse_declare(),
            Token::KwRedim => self.parse_redim(),
            Token::KwErase => self.parse_erase(),
            Token::KwOption => self.parse_option_base(),
            Token::KwSwap => self.parse_swap(),
            Token::KwData => self.parse_data(),
            Token::KwRead => self.parse_read(),
            Token::KwRestore => self.parse_restore(),
            Token::KwOpen => self.parse_open(),
            Token::KwClose => self.parse_close(),
            Token::KwOn => self.parse_on(),
            Token::KwResume => self.parse_resume(),
            Token::KwRandomize => {
                self.advance();
                // RANDOMIZE [TIMER | expr] — just skip for now
                if !self.at_stmt_end() {
                    if matches!(self.peek(), Token::KwTimer) {
                        self.advance();
                    } else {
                        let _ = self.parse_expr()?;
                    }
                }
                Ok(Stmt::Rem) // treat as no-op for now
            }
            Token::KwRem => {
                self.advance();
                Ok(Stmt::Rem)
            }
            Token::Identifier { .. } => self.parse_assignment_or_call(),
            _ => {
                let tok = self.peek().clone();
                Err(ParseError::Unexpected {
                    line: self.current_line(),
                    token: format!("{tok:?}"),
                })
            }
        }
    }

    fn parse_print(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume PRINT

        // Check for PRINT #n (file print)
        if matches!(self.peek(), Token::Hash) {
            return self.parse_file_print();
        }

        let mut items = Vec::new();
        let mut trailing = PrintSep::Newline;

        if self.at_stmt_end() {
            return Ok(Stmt::Print(PrintStmt { items, trailing }));
        }

        // Check for PRINT USING
        if matches!(self.peek(), Token::KwUsing) {
            // For now, treat PRINT USING as regular PRINT (simplified)
            self.advance(); // skip USING
            let _format = self.parse_expr()?; // skip format string
            self.expect(Token::Semicolon)?; // skip ;
        }

        loop {
            if self.at_stmt_end() {
                break;
            }

            match self.peek() {
                Token::Semicolon => {
                    self.advance();
                    trailing = PrintSep::Semicolon;
                    if self.at_stmt_end() {
                        break;
                    }
                    continue;
                }
                Token::Comma => {
                    self.advance();
                    items.push(PrintItem::Comma);
                    trailing = PrintSep::Comma;
                    if self.at_stmt_end() {
                        break;
                    }
                    continue;
                }
                Token::KwTab => {
                    self.advance();
                    self.expect(Token::LeftParen)?;
                    let expr = self.parse_expr()?;
                    self.expect(Token::RightParen)?;
                    items.push(PrintItem::Tab(expr));
                    trailing = PrintSep::Newline;
                }
                Token::KwSpc => {
                    self.advance();
                    self.expect(Token::LeftParen)?;
                    let expr = self.parse_expr()?;
                    self.expect(Token::RightParen)?;
                    items.push(PrintItem::Spc(expr));
                    trailing = PrintSep::Newline;
                }
                _ => {
                    let expr = self.parse_expr()?;
                    items.push(PrintItem::Expr(expr));
                    trailing = PrintSep::Newline;
                }
            }
        }

        Ok(Stmt::Print(PrintStmt { items, trailing }))
    }

    fn parse_file_print(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume #
        let file_num = self.parse_expr()?;
        self.expect(Token::Comma)?;

        let mut items = Vec::new();
        let mut trailing = PrintSep::Newline;

        while !self.at_stmt_end() {
            match self.peek() {
                Token::Semicolon => {
                    self.advance();
                    trailing = PrintSep::Semicolon;
                    if self.at_stmt_end() {
                        break;
                    }
                    continue;
                }
                Token::Comma => {
                    self.advance();
                    items.push(PrintItem::Comma);
                    trailing = PrintSep::Comma;
                    if self.at_stmt_end() {
                        break;
                    }
                    continue;
                }
                _ => {
                    let expr = self.parse_expr()?;
                    items.push(PrintItem::Expr(expr));
                    trailing = PrintSep::Newline;
                }
            }
        }

        Ok(Stmt::PrintFile(FilePrintStmt {
            file_num,
            items,
            trailing,
        }))
    }

    fn parse_assignment(&mut self) -> Result<Stmt, ParseError> {
        let var = self.parse_variable()?;
        self.expect(Token::Equal)?;
        let expr = self.parse_expr()?;
        Ok(Stmt::Let { var, expr })
    }

    fn parse_assignment_or_call(&mut self) -> Result<Stmt, ParseError> {
        // This is only called when peek() is Token::Identifier
        let Token::Identifier { name, suffix } = self.peek().clone() else {
            // Unreachable: parse_statement only calls this for Identifier tokens
            let expr = self.parse_expr()?;
            return Ok(Stmt::ExprStmt(expr));
        };

        self.advance();

        // Check for array assignment or sub call with parens: name(...)
        if matches!(self.peek(), Token::LeftParen) {
            // Parse the argument list; then check if = follows (array assignment)
            // or not (sub call)
            let paren_save = self.pos;
            self.advance(); // consume (
            let mut indices = Vec::new();
            if !matches!(self.peek(), Token::RightParen) {
                indices.push(self.parse_expr()?);
                while matches!(self.peek(), Token::Comma) {
                    self.advance();
                    indices.push(self.parse_expr()?);
                }
            }
            self.expect(Token::RightParen)?;

            if matches!(self.peek(), Token::Equal) {
                // Array assignment: name(indices) = expr
                self.advance(); // consume =
                let expr = self.parse_expr()?;
                return Ok(Stmt::Let {
                    var: Variable {
                        name: name.clone(),
                        suffix,
                    },
                    expr: Expr::BinaryOp {
                        left: Box::new(Expr::ArrayIndex {
                            name,
                            suffix,
                            indices,
                        }),
                        op: BinOp::Eq, // marker — interpreter handles this specially
                        right: Box::new(expr),
                    },
                });
            }

            // SUB call with parenthesized args — re-parse since indices
            // may have consumed expressions differently than args would
            self.pos = paren_save;
            self.advance(); // consume (
            let mut args = Vec::new();
            if !matches!(self.peek(), Token::RightParen) {
                args.push(self.parse_expr()?);
                while matches!(self.peek(), Token::Comma) {
                    self.advance();
                    args.push(self.parse_expr()?);
                }
            }
            self.expect(Token::RightParen)?;
            return Ok(Stmt::Call { name, args });
        }

        // Simple assignment: name = expr
        if matches!(self.peek(), Token::Equal) {
            self.advance();
            let expr = self.parse_expr()?;
            return Ok(Stmt::Let {
                var: Variable { name, suffix },
                expr,
            });
        }

        // SUB call without parens: name arg1, arg2, ...
        let mut args = Vec::new();
        if !self.at_stmt_end() {
            args.push(self.parse_expr()?);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                args.push(self.parse_expr()?);
            }
        }
        Ok(Stmt::Call { name, args })
    }

    fn parse_dim(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume DIM
        let shared = if matches!(self.peek(), Token::KwShared) {
            self.advance();
            true
        } else {
            false
        };
        let _ = shared; // TODO: use in Phase 2

        let mut decls = vec![self.parse_dim_decl()?];
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            decls.push(self.parse_dim_decl()?);
        }
        Ok(Stmt::Dim(decls))
    }

    fn parse_dim_decl(&mut self) -> Result<DimDecl, ParseError> {
        let (name, suffix) = self.expect_identifier()?;

        let dimensions = if matches!(self.peek(), Token::LeftParen) {
            self.advance();
            let mut dims = vec![self.parse_dim_range()?];
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                dims.push(self.parse_dim_range()?);
            }
            self.expect(Token::RightParen)?;
            Some(dims)
        } else {
            None
        };

        let as_type = if matches!(self.peek(), Token::KwAs) {
            self.advance();
            Some(self.parse_type_keyword()?)
        } else {
            None
        };

        Ok(DimDecl {
            name,
            suffix,
            as_type,
            dimensions,
        })
    }

    fn parse_dim_range(&mut self) -> Result<(Expr, Option<Expr>), ParseError> {
        let first = self.parse_expr()?;
        if matches!(self.peek(), Token::KwTo) {
            self.advance();
            let upper = self.parse_expr()?;
            Ok((first, Some(upper)))
        } else {
            Ok((first, None))
        }
    }

    fn parse_const(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume CONST
        let (name, _suffix) = self.expect_identifier()?;
        self.expect(Token::Equal)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Const { name, value })
    }

    fn parse_input(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume INPUT

        // Check for INPUT #n (file input)
        if matches!(self.peek(), Token::Hash) {
            return self.parse_file_input();
        }

        let mut prompt = None;

        // Check for prompt string
        if let Token::StringLiteral(s) = self.peek().clone() {
            self.advance();
            if matches!(self.peek(), Token::Semicolon | Token::Comma) {
                self.advance();
            }
            prompt = Some(s);
        }

        let mut vars = vec![self.parse_variable()?];
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            vars.push(self.parse_variable()?);
        }

        Ok(Stmt::Input(InputStmt { prompt, vars }))
    }

    fn parse_file_input(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume #
        let file_num = self.parse_expr()?;
        self.expect(Token::Comma)?;
        let mut vars = vec![self.parse_variable()?];
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            vars.push(self.parse_variable()?);
        }
        Ok(Stmt::InputFile(FileInputStmt { file_num, vars }))
    }

    fn parse_line_input(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume LINE INPUT

        // Check for LINE INPUT #n
        if matches!(self.peek(), Token::Hash) {
            self.advance();
            let file_num = self.parse_expr()?;
            self.expect(Token::Comma)?;
            let var = self.parse_variable()?;
            return Ok(Stmt::LineInputFile { file_num, var });
        }

        let mut prompt = None;
        if let Token::StringLiteral(s) = self.peek().clone() {
            self.advance();
            if matches!(self.peek(), Token::Semicolon | Token::Comma) {
                self.advance();
            }
            prompt = Some(s);
        }

        let var = self.parse_variable()?;
        Ok(Stmt::LineInput { prompt, var })
    }

    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume IF
        let condition = self.parse_expr()?;
        self.expect(Token::KwThen)?;

        // Determine: block IF or single-line IF
        // Block IF: THEN is followed by newline (or comment/EOF)
        if matches!(self.peek(), Token::Newline | Token::Eof) {
            // Block IF
            self.skip_newlines();
            let then_body = self.parse_body_until(&[
                Token::KwElseIf,
                Token::KwElse,
                Token::KwEndIf,
            ])?;

            let mut elseif_clauses = Vec::new();
            while matches!(self.peek(), Token::KwElseIf) {
                self.advance();
                let cond = self.parse_expr()?;
                self.expect(Token::KwThen)?;
                self.skip_newlines();
                let body = self.parse_body_until(&[
                    Token::KwElseIf,
                    Token::KwElse,
                    Token::KwEndIf,
                ])?;
                elseif_clauses.push((cond, body));
            }

            let else_body = if matches!(self.peek(), Token::KwElse) {
                self.advance();
                self.skip_newlines();
                Some(self.parse_body_until(&[Token::KwEndIf])?)
            } else {
                None
            };

            self.expect(Token::KwEndIf)?;

            Ok(Stmt::If(IfStmt {
                condition,
                then_body,
                elseif_clauses,
                else_body,
            }))
        } else {
            // Single-line IF
            let then_body = self.parse_single_line_stmts()?;

            let else_body = if matches!(self.peek(), Token::KwElse) {
                self.advance();
                Some(self.parse_single_line_stmts()?)
            } else {
                None
            };

            Ok(Stmt::If(IfStmt {
                condition,
                then_body,
                elseif_clauses: Vec::new(),
                else_body,
            }))
        }
    }

    fn parse_single_line_stmts(&mut self) -> Result<Vec<LabeledStmt>, ParseError> {
        let mut stmts = Vec::new();
        loop {
            if self.at_stmt_end() || matches!(self.peek(), Token::KwElse) {
                break;
            }
            let line = self.current_line();
            let stmt = self.parse_statement()?;
            stmts.push(LabeledStmt {
                label: None,
                stmt,
                line,
            });
            if matches!(self.peek(), Token::Colon) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(stmts)
    }

    fn parse_for(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume FOR
        let var = self.parse_variable()?;
        self.expect(Token::Equal)?;
        let start = self.parse_expr()?;
        self.expect(Token::KwTo)?;
        let end = self.parse_expr()?;

        let step = if matches!(self.peek(), Token::KwStep) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        self.skip_newlines();
        let body = self.parse_body_until(&[Token::KwNext])?;
        self.expect(Token::KwNext)?;

        // Optional variable name after NEXT
        if let Token::Identifier { .. } = self.peek() {
            self.advance();
        }

        Ok(Stmt::For(ForStmt {
            var,
            start,
            end,
            step,
            body,
        }))
    }

    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume WHILE
        let condition = self.parse_expr()?;
        self.skip_newlines();
        let body = self.parse_body_until(&[Token::KwWend])?;
        self.expect(Token::KwWend)?;
        Ok(Stmt::WhileWend { condition, body })
    }

    fn parse_do(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume DO

        // DO WHILE condition / DO UNTIL condition
        if matches!(self.peek(), Token::KwWhile | Token::KwUntil) {
            let is_while = matches!(self.peek(), Token::KwWhile);
            self.advance();
            let condition = self.parse_expr()?;
            self.skip_newlines();
            let body = self.parse_body_until(&[Token::KwLoop])?;
            self.expect(Token::KwLoop)?;
            return Ok(Stmt::DoLoop(DoLoopStmt {
                condition: Some(condition),
                check_at_top: true,
                is_while,
                body,
            }));
        }

        // DO ... LOOP [WHILE|UNTIL condition]
        self.skip_newlines();
        let body = self.parse_body_until(&[Token::KwLoop])?;
        self.expect(Token::KwLoop)?;

        if matches!(self.peek(), Token::KwWhile | Token::KwUntil) {
            let is_while = matches!(self.peek(), Token::KwWhile);
            self.advance();
            let condition = self.parse_expr()?;
            return Ok(Stmt::DoLoop(DoLoopStmt {
                condition: Some(condition),
                check_at_top: false,
                is_while,
                body,
            }));
        }

        // Infinite DO...LOOP
        Ok(Stmt::DoLoop(DoLoopStmt {
            condition: None,
            check_at_top: false,
            is_while: true,
            body,
        }))
    }

    fn parse_select(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume SELECT (CASE already consumed by compound keyword)
        // If CASE wasn't consumed as part of compound keyword, consume it now
        if matches!(self.peek(), Token::KwCase) {
            self.advance();
        }
        let expr = self.parse_expr()?;
        self.skip_newlines();

        let mut cases = Vec::new();
        let mut else_body = None;

        while matches!(self.peek(), Token::KwCase) {
            self.advance(); // consume CASE

            // CASE ELSE
            if matches!(self.peek(), Token::KwElse) {
                self.advance();
                self.skip_newlines();
                else_body = Some(self.parse_body_until(&[Token::KwEndSelect])?);
                break;
            }

            // Parse case tests
            let mut tests = vec![self.parse_case_test()?];
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                tests.push(self.parse_case_test()?);
            }

            self.skip_newlines();
            let body = self.parse_body_until(&[Token::KwCase, Token::KwEndSelect])?;
            cases.push(CaseClause { tests, body });
        }

        self.expect(Token::KwEndSelect)?;
        Ok(Stmt::SelectCase(SelectCaseStmt {
            expr,
            cases,
            else_body,
        }))
    }

    fn parse_case_test(&mut self) -> Result<CaseTest, ParseError> {
        // CASE IS > 5
        if matches!(self.peek(), Token::KwIs) {
            self.advance();
            let op = self.parse_compare_op()?;
            let expr = self.parse_expr()?;
            return Ok(CaseTest::Comparison(op, expr));
        }

        let expr = self.parse_expr()?;

        // CASE 1 TO 10
        if matches!(self.peek(), Token::KwTo) {
            self.advance();
            let upper = self.parse_expr()?;
            return Ok(CaseTest::Range(expr, upper));
        }

        Ok(CaseTest::Value(expr))
    }

    fn parse_compare_op(&mut self) -> Result<CompareOp, ParseError> {
        let op = match self.peek() {
            Token::Equal => CompareOp::Eq,
            Token::NotEqual => CompareOp::Ne,
            Token::Less => CompareOp::Lt,
            Token::Greater => CompareOp::Gt,
            Token::LessEqual => CompareOp::Le,
            Token::GreaterEqual => CompareOp::Ge,
            _ => {
                return Err(ParseError::Expected {
                    line: self.current_line(),
                    expected: "comparison operator".into(),
                    found: format!("{:?}", self.peek()),
                });
            }
        };
        self.advance();
        Ok(op)
    }

    fn parse_exit(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume EXIT
        match self.peek() {
            Token::KwFor => {
                self.advance();
                Ok(Stmt::ExitFor)
            }
            Token::KwDo => {
                self.advance();
                Ok(Stmt::ExitDo)
            }
            Token::KwSub => {
                self.advance();
                Ok(Stmt::ExitSub)
            }
            Token::KwFunction => {
                self.advance();
                Ok(Stmt::ExitFunction)
            }
            _ => Err(ParseError::Expected {
                line: self.current_line(),
                expected: "FOR, DO, SUB, or FUNCTION after EXIT".into(),
                found: format!("{:?}", self.peek()),
            }),
        }
    }

    fn parse_sub_def(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume SUB
        let (name, _) = self.expect_identifier()?;

        let is_static = false; // TODO

        let params = if matches!(self.peek(), Token::LeftParen) {
            self.advance();
            let p = self.parse_param_list()?;
            self.expect(Token::RightParen)?;
            p
        } else {
            Vec::new()
        };

        self.skip_newlines();
        let body = self.parse_body_until(&[Token::KwEndSub])?;
        self.expect(Token::KwEndSub)?;

        Ok(Stmt::SubDef(SubDef {
            name,
            params,
            body,
            is_static,
        }))
    }

    fn parse_function_def(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume FUNCTION
        let (name, suffix) = self.expect_identifier()?;

        let is_static = false; // TODO

        let params = if matches!(self.peek(), Token::LeftParen) {
            self.advance();
            let p = self.parse_param_list()?;
            self.expect(Token::RightParen)?;
            p
        } else {
            Vec::new()
        };

        let as_type = if matches!(self.peek(), Token::KwAs) {
            self.advance();
            Some(self.parse_type_keyword()?)
        } else {
            None
        };

        self.skip_newlines();
        let body = self.parse_body_until(&[Token::KwEndFunction])?;
        self.expect(Token::KwEndFunction)?;

        Ok(Stmt::FunctionDef(FunctionDef {
            name,
            suffix,
            params,
            as_type,
            body,
            is_static,
        }))
    }

    fn parse_param_list(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if matches!(self.peek(), Token::RightParen) {
            return Ok(params);
        }
        params.push(self.parse_param()?);
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            params.push(self.parse_param()?);
        }
        Ok(params)
    }

    fn parse_param(&mut self) -> Result<Param, ParseError> {
        let by_val = if matches!(self.peek(), Token::KwByVal) {
            self.advance();
            true
        } else {
            false
        };

        let (name, suffix) = self.expect_identifier()?;

        // Check for array param: name()
        let is_array = if matches!(self.peek(), Token::LeftParen) {
            self.advance();
            self.expect(Token::RightParen)?;
            true
        } else {
            false
        };

        let as_type = if matches!(self.peek(), Token::KwAs) {
            self.advance();
            Some(self.parse_type_keyword()?)
        } else {
            None
        };

        Ok(Param {
            name,
            suffix,
            as_type,
            by_val,
            is_array,
        })
    }

    fn parse_call(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume CALL
        let (name, _) = self.expect_identifier()?;

        let args = if matches!(self.peek(), Token::LeftParen) {
            self.advance();
            let mut args = Vec::new();
            if !matches!(self.peek(), Token::RightParen) {
                args.push(self.parse_expr()?);
                while matches!(self.peek(), Token::Comma) {
                    self.advance();
                    args.push(self.parse_expr()?);
                }
            }
            self.expect(Token::RightParen)?;
            args
        } else {
            Vec::new()
        };

        Ok(Stmt::Call { name, args })
    }

    fn parse_declare(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume DECLARE
        let is_function = match self.peek() {
            Token::KwFunction => {
                self.advance();
                true
            }
            Token::KwSub => {
                self.advance();
                false
            }
            _ => {
                return Err(ParseError::Expected {
                    line: self.current_line(),
                    expected: "FUNCTION or SUB".into(),
                    found: format!("{:?}", self.peek()),
                });
            }
        };

        let (name, suffix) = self.expect_identifier()?;

        let params = if matches!(self.peek(), Token::LeftParen) {
            self.advance();
            let p = self.parse_param_list()?;
            self.expect(Token::RightParen)?;
            p
        } else {
            Vec::new()
        };

        Ok(Stmt::Declare(DeclareStmt {
            is_function,
            name,
            suffix,
            params,
        }))
    }

    fn parse_redim(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume REDIM
        let preserve = if matches!(self.peek(), Token::KwPreserve) {
            self.advance();
            true
        } else {
            false
        };

        let mut decls = vec![self.parse_dim_decl()?];
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            decls.push(self.parse_dim_decl()?);
        }

        Ok(Stmt::Redim { preserve, decls })
    }

    fn parse_erase(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume ERASE
        let mut names = Vec::new();
        let (name, _) = self.expect_identifier()?;
        names.push(name);
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            let (name, _) = self.expect_identifier()?;
            names.push(name);
        }
        Ok(Stmt::Erase(names))
    }

    fn parse_option_base(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume OPTION (BASE was consumed by compound keyword)
        if matches!(self.peek(), Token::KwBase) {
            self.advance();
        }
        let n = match self.peek() {
            Token::IntegerLiteral(n) => {
                let n = *n as i32;
                self.advance();
                n
            }
            _ => {
                return Err(ParseError::Expected {
                    line: self.current_line(),
                    expected: "0 or 1".into(),
                    found: format!("{:?}", self.peek()),
                });
            }
        };
        Ok(Stmt::OptionBase(n))
    }

    fn parse_swap(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume SWAP
        let a = self.parse_variable()?;
        self.expect(Token::Comma)?;
        let b = self.parse_variable()?;
        Ok(Stmt::Swap { a, b })
    }

    fn parse_data(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume DATA
        let mut items = Vec::new();
        loop {
            match self.peek() {
                Token::StringLiteral(s) => {
                    items.push(DataItem::Str(s.clone()));
                    self.advance();
                }
                Token::IntegerLiteral(n) => {
                    items.push(DataItem::Number(*n as f64));
                    self.advance();
                }
                Token::DoubleLiteral(n) => {
                    items.push(DataItem::Number(*n));
                    self.advance();
                }
                Token::Minus => {
                    self.advance();
                    match self.peek() {
                        Token::IntegerLiteral(n) => {
                            items.push(DataItem::Number(-(*n as f64)));
                            self.advance();
                        }
                        Token::DoubleLiteral(n) => {
                            items.push(DataItem::Number(-*n));
                            self.advance();
                        }
                        _ => {
                            return Err(ParseError::Expected {
                                line: self.current_line(),
                                expected: "number after minus in DATA".into(),
                                found: format!("{:?}", self.peek()),
                            });
                        }
                    }
                }
                Token::Identifier { name, .. } => {
                    // Unquoted string in DATA
                    items.push(DataItem::Str(name.clone()));
                    self.advance();
                }
                _ => break,
            }
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(Stmt::Data(items))
    }

    fn parse_read(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume READ
        let mut vars = vec![self.parse_variable()?];
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            vars.push(self.parse_variable()?);
        }
        Ok(Stmt::Read(vars))
    }

    fn parse_restore(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume RESTORE
        let label = if !self.at_stmt_end() {
            Some(self.parse_label()?)
        } else {
            None
        };
        Ok(Stmt::Restore(label))
    }

    fn parse_open(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume OPEN
        let filename = self.parse_expr()?;
        self.expect(Token::KwFor)?;

        let mode = match self.peek() {
            Token::KwInput => {
                self.advance();
                FileMode::Input
            }
            Token::KwOutput => {
                self.advance();
                FileMode::Output
            }
            Token::KwAppend => {
                self.advance();
                FileMode::Append
            }
            Token::KwRandom => {
                self.advance();
                FileMode::Random
            }
            Token::KwBinary => {
                self.advance();
                FileMode::Binary
            }
            _ => {
                return Err(ParseError::Expected {
                    line: self.current_line(),
                    expected: "INPUT, OUTPUT, APPEND, RANDOM, or BINARY".into(),
                    found: format!("{:?}", self.peek()),
                });
            }
        };

        self.expect(Token::KwAs)?;
        // Optional #
        if matches!(self.peek(), Token::Hash) {
            self.advance();
        }
        let file_num = self.parse_expr()?;

        let rec_len = if matches!(self.peek(), Token::KwLen) {
            self.advance();
            self.expect(Token::Equal)?;
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(Stmt::Open(OpenStmt {
            filename,
            mode,
            file_num,
            rec_len,
        }))
    }

    fn parse_close(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume CLOSE
        let mut file_nums = Vec::new();
        if !self.at_stmt_end() {
            // Optional #
            if matches!(self.peek(), Token::Hash) {
                self.advance();
            }
            file_nums.push(self.parse_expr()?);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                if matches!(self.peek(), Token::Hash) {
                    self.advance();
                }
                file_nums.push(self.parse_expr()?);
            }
        }
        Ok(Stmt::Close(file_nums))
    }

    fn parse_on(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume ON
        if matches!(self.peek(), Token::KwError) {
            self.advance();
            self.expect(Token::KwGoto)?;
            // ON ERROR GOTO 0 disables error handling
            if let Token::IntegerLiteral(0) = self.peek() {
                self.advance();
                return Ok(Stmt::OnErrorGoto(None));
            }
            let label = self.parse_label()?;
            return Ok(Stmt::OnErrorGoto(Some(label)));
        }
        // ON n GOTO/GOSUB — simplified: parse as expression
        Err(ParseError::General {
            line: self.current_line(),
            msg: "ON...GOTO/GOSUB not yet supported".into(),
        })
    }

    fn parse_resume(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume RESUME
        if self.at_stmt_end() {
            return Ok(Stmt::Resume(ResumeTarget::Default));
        }
        if matches!(self.peek(), Token::KwNext) {
            self.advance();
            return Ok(Stmt::Resume(ResumeTarget::Next));
        }
        let label = self.parse_label()?;
        Ok(Stmt::Resume(ResumeTarget::Label(label)))
    }

    // ==================== Expression parsing ====================

    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_imp_expr()
    }

    fn parse_imp_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_eqv_expr()?;
        while matches!(self.peek(), Token::KwImp) {
            self.advance();
            let right = self.parse_eqv_expr()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::Imp,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_eqv_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_xor_expr()?;
        while matches!(self.peek(), Token::KwEqv) {
            self.advance();
            let right = self.parse_xor_expr()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::Eqv,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_xor_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_or_expr()?;
        while matches!(self.peek(), Token::KwXor) {
            self.advance();
            let right = self.parse_or_expr()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::Xor,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and_expr()?;
        while matches!(self.peek(), Token::KwOr) {
            self.advance();
            let right = self.parse_and_expr()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::Or,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not_expr()?;
        while matches!(self.peek(), Token::KwAnd) {
            self.advance();
            let right = self.parse_not_expr()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::And,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_not_expr(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek(), Token::KwNot) {
            self.advance();
            let operand = self.parse_not_expr()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            });
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Token::Equal => BinOp::Eq,
                Token::NotEqual => BinOp::Ne,
                Token::Less => BinOp::Lt,
                Token::Greater => BinOp::Gt,
                Token::LessEqual => BinOp::Le,
                Token::GreaterEqual => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_mod_expr()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mod_expr()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_mod_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_intdiv()?;
        while matches!(self.peek(), Token::KwMod) {
            self.advance();
            let right = self.parse_intdiv()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::Mod,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_intdiv(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;
        while matches!(self.peek(), Token::Backslash) {
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::IntDiv,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Token::Minus => {
                self.advance();
                let operand = self.parse_power()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                })
            }
            Token::Plus => {
                self.advance();
                self.parse_power()
            }
            _ => self.parse_power(),
        }
    }

    fn parse_power(&mut self) -> Result<Expr, ParseError> {
        let base = self.parse_primary()?;
        if matches!(self.peek(), Token::Caret) {
            self.advance();
            let exp = self.parse_unary()?; // right-associative
            Ok(Expr::BinaryOp {
                left: Box::new(base),
                op: BinOp::Pow,
                right: Box::new(exp),
            })
        } else {
            Ok(base)
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::IntegerLiteral(n) => {
                self.advance();
                Ok(Expr::IntegerLit(n))
            }
            Token::DoubleLiteral(n) => {
                self.advance();
                Ok(Expr::DoubleLit(n))
            }
            Token::StringLiteral(s) => {
                self.advance();
                Ok(Expr::StringLit(s))
            }
            Token::LeftParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(Token::RightParen)?;
                Ok(Expr::Paren(Box::new(expr)))
            }
            Token::Identifier { name, suffix } => {
                self.advance();

                // Check for function call / array index
                if matches!(self.peek(), Token::LeftParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Token::RightParen) {
                        args.push(self.parse_expr()?);
                        while matches!(self.peek(), Token::Comma) {
                            self.advance();
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.expect(Token::RightParen)?;

                    // Build canonical function name with suffix
                    let func_name = match suffix {
                        Some(s) => format!("{}{}", name, s.to_char()),
                        None => name.clone(),
                    };

                    Ok(Expr::FunctionCall {
                        name: func_name,
                        suffix,
                        args,
                    })
                } else {
                    Ok(Expr::Variable(Variable { name, suffix }))
                }
            }
            Token::KwTimer => {
                self.advance();
                Ok(Expr::FunctionCall {
                    name: "TIMER".into(),
                    suffix: None,
                    args: vec![],
                })
            }
            Token::KwFreefile => {
                self.advance();
                Ok(Expr::FunctionCall {
                    name: "FREEFILE".into(),
                    suffix: None,
                    args: vec![],
                })
            }
            // Keywords that double as built-in functions
            ref tok if self.is_keyword_function(tok) => {
                let name = self.keyword_function_name(tok);
                self.advance();
                if matches!(self.peek(), Token::LeftParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Token::RightParen) {
                        args.push(self.parse_expr()?);
                        while matches!(self.peek(), Token::Comma) {
                            self.advance();
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.expect(Token::RightParen)?;
                    Ok(Expr::FunctionCall {
                        name,
                        suffix: None,
                        args,
                    })
                } else {
                    // No parens — treat as 0-arg function call
                    Ok(Expr::FunctionCall {
                        name,
                        suffix: None,
                        args: vec![],
                    })
                }
            }
            _ => Err(ParseError::Unexpected {
                line: self.current_line(),
                token: format!("{:?}", self.peek()),
            }),
        }
    }

    fn is_keyword_function(&self, tok: &Token) -> bool {
        matches!(
            tok,
            Token::KwLen | Token::KwString
        )
    }

    fn keyword_function_name(&self, tok: &Token) -> String {
        match tok {
            Token::KwLen => "LEN".into(),
            Token::KwString => "STRING$".into(),
            _ => "UNKNOWN".into(),
        }
    }

    // ==================== Helpers ====================

    fn parse_variable(&mut self) -> Result<Variable, ParseError> {
        let (name, suffix) = self.expect_identifier()?;
        Ok(Variable { name, suffix })
    }

    fn parse_label(&mut self) -> Result<Label, ParseError> {
        match self.peek().clone() {
            Token::IntegerLiteral(n) => {
                self.advance();
                Ok(Label::Number(n as u32))
            }
            Token::LineNumber(n) => {
                self.advance();
                Ok(Label::Number(n))
            }
            Token::Identifier { name, .. } => {
                let name = name.clone();
                self.advance();
                Ok(Label::Name(name))
            }
            _ => Err(ParseError::Expected {
                line: self.current_line(),
                expected: "label or line number".into(),
                found: format!("{:?}", self.peek()),
            }),
        }
    }

    fn parse_type_keyword(&mut self) -> Result<BasicType, ParseError> {
        let ty = match self.peek() {
            Token::KwInteger => BasicType::Integer,
            Token::KwLong => BasicType::Long,
            Token::KwSingle => BasicType::Single,
            Token::KwDouble => BasicType::Double,
            Token::KwString => BasicType::String,
            _ => {
                return Err(ParseError::Expected {
                    line: self.current_line(),
                    expected: "type name".into(),
                    found: format!("{:?}", self.peek()),
                });
            }
        };
        self.advance();
        Ok(ty)
    }

    fn parse_body_until(&mut self, terminators: &[Token]) -> Result<Vec<LabeledStmt>, ParseError> {
        let mut stmts = Vec::new();
        while !self.at_end() && !terminators.iter().any(|t| self.peek_matches(t)) {
            let stmt = self.parse_labeled_stmt()?;
            stmts.push(stmt);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    fn peek_matches(&self, expected: &Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(expected)
    }

    fn expect_identifier(&mut self) -> Result<(String, Option<TypeSuffix>), ParseError> {
        match self.peek().clone() {
            Token::Identifier { name, suffix } => {
                self.advance();
                Ok((name, suffix))
            }
            _ => Err(ParseError::Expected {
                line: self.current_line(),
                expected: "identifier".into(),
                found: format!("{:?}", self.peek()),
            }),
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        if self.peek_matches(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::Expected {
                line: self.current_line(),
                expected: format!("{expected:?}"),
                found: format!("{:?}", self.peek()),
            })
        }
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .map(|st| &st.token)
            .unwrap_or(&Token::Eof)
    }

    fn peek_at(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset).map(|st| &st.token)
    }

    fn advance(&mut self) -> &SpannedToken {
        let tok = &self.tokens[self.pos];
        self.pos += 1;
        tok
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len() || matches!(self.peek(), Token::Eof)
    }

    fn at_stmt_end(&self) -> bool {
        matches!(self.peek(), Token::Newline | Token::Eof | Token::Colon | Token::KwElse)
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Token::Newline) {
            self.advance();
        }
    }

    fn current_line(&self) -> usize {
        self.tokens
            .get(self.pos)
            .map(|st| st.span.line)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(s: &str) -> Program {
        let tokens = Lexer::new(s).tokenize().unwrap();
        Parser::new(tokens).parse_program().unwrap()
    }

    #[test]
    fn test_print_literal() {
        let prog = parse("PRINT 42");
        assert_eq!(prog.statements.len(), 1);
        assert!(matches!(prog.statements[0].stmt, Stmt::Print(_)));
    }

    #[test]
    fn test_let_assignment() {
        let prog = parse("LET x = 10");
        assert_eq!(prog.statements.len(), 1);
        assert!(matches!(prog.statements[0].stmt, Stmt::Let { .. }));
    }

    #[test]
    fn test_implicit_assignment() {
        let prog = parse("x = 10");
        assert_eq!(prog.statements.len(), 1);
        assert!(matches!(prog.statements[0].stmt, Stmt::Let { .. }));
    }

    #[test]
    fn test_if_block() {
        let prog = parse("IF x > 0 THEN\nPRINT 1\nEND IF");
        assert_eq!(prog.statements.len(), 1);
        assert!(matches!(prog.statements[0].stmt, Stmt::If(_)));
    }

    #[test]
    fn test_for_loop() {
        let prog = parse("FOR i = 1 TO 10\nPRINT i\nNEXT i");
        assert_eq!(prog.statements.len(), 1);
        assert!(matches!(prog.statements[0].stmt, Stmt::For(_)));
    }

    #[test]
    fn test_operator_precedence() {
        let prog = parse("PRINT 2 + 3 * 4");
        if let Stmt::Print(ps) = &prog.statements[0].stmt
            && let PrintItem::Expr(expr) = &ps.items[0]
        {
            // Should be Add(2, Mul(3, 4))
            assert!(matches!(expr, Expr::BinaryOp { op: BinOp::Add, .. }));
            if let Expr::BinaryOp { right, .. } = expr {
                assert!(matches!(**right, Expr::BinaryOp { op: BinOp::Mul, .. }));
            }
        }
    }
}
