pub mod ast;

use crate::lexer::Token;
pub use ast::{Program, Statement, Expr, BinOp, Param, UnOp, Type};

#[derive(Debug, PartialEq, Clone)]
pub enum ParseError<'a> {
    UnexpectedToken {
        expected: &'static str,
        found: Option<Token<'a>>,
    },
    ExpectedIdentifier {
        found: Option<Token<'a>>,
    },
    InvalidExpression {
        found: Option<Token<'a>>,
    },
}

pub struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    pos: usize,
    struct_literal_allowed: bool,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self { tokens, pos: 0, struct_literal_allowed: true }
    }

    fn peek(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token<'a>> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn consume(&mut self, expected: Token<'a>, err_msg: &'static str) -> Result<Token<'a>, ParseError<'a>> {
        match self.peek() {
            Some(tok) if tok == &expected => Ok(self.advance().unwrap()),
            found => Err(ParseError::UnexpectedToken {
                expected: err_msg,
                found: found.cloned(),
            }),
        }
    }

    fn parse_type(&mut self) -> Result<Type<'a>, ParseError<'a>> {
        match self.peek() {
            Some(Token::Ampersand) => {
                self.advance();
                let is_mut = if self.peek() == Some(&Token::Mut) {
                    self.advance();
                    true
                } else {
                    false
                };
                let ty = self.parse_type()?;
                Ok(Type::Ref {
                    is_mut,
                    ty: Box::new(ty),
                })
            }
            Some(Token::Ident(ty)) => {
                let ty_str = *ty;
                self.advance();
                Ok(Type::Ident(ty_str))
            }
            found => Err(ParseError::UnexpectedToken {
                expected: "type name or '&'",
                found: found.cloned(),
            }),
        }
    }

    fn parse_fn_params(&mut self) -> Result<Vec<Param<'a>>, ParseError<'a>> {
        self.consume(Token::LParen, "(")?;
        let mut params = Vec::new();
        if self.peek() != Some(&Token::RParen) {
            loop {
                let _is_param_mut = if self.peek() == Some(&Token::Mut) {
                    self.advance();
                    true
                } else {
                    false
                };
                let param_name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                self.consume(Token::Colon, ":")?;
                let param_ty = self.parse_type()?;
                params.push(Param { name: param_name, ty: param_ty });
                if self.peek() == Some(&Token::Comma) {
                    self.advance();
                    if self.peek() == Some(&Token::RParen) {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        self.consume(Token::RParen, ")")?;
        Ok(params)
    }

    pub fn parse(&mut self) -> Result<Program<'a>, ParseError<'a>> {
        let mut statements = Vec::new();
        while self.peek().is_some() {
            statements.push(self.parse_statement()?);
        }
        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement<'a>, ParseError<'a>> {
        let is_pub = if self.peek() == Some(&Token::Pub) {
            self.advance();
            true
        } else {
            false
        };

        if is_pub {
            match self.peek() {
                Some(Token::Fn) | Some(Token::Struct) | Some(Token::Extern) => {}
                found => return Err(ParseError::UnexpectedToken {
                    expected: "fn, struct or extern after pub",
                    found: found.cloned(),
                }),
            }
        }

        match self.peek() {
            Some(Token::Fn) => {
                self.advance();
                let name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                let params = self.parse_fn_params()?;
                let ret_type = if self.peek() == Some(&Token::Arrow) {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                self.consume(Token::LBrace, "{")?;
                let mut body = Vec::new();
                while self.peek().is_some() && self.peek() != Some(&Token::RBrace) {
                    body.push(self.parse_statement()?);
                }
                self.consume(Token::RBrace, "}")?;
                Ok(Statement::Function { name, params, ret_type, body, is_pub })
            }
            Some(Token::Extern) => {
                self.advance();
                self.consume(Token::Fn, "fn")?;
                let name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                let params = self.parse_fn_params()?;
                let ret_type = if self.peek() == Some(&Token::Arrow) {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                self.consume(Token::Semi, ";")?;
                Ok(Statement::ExternFunction { name, params, ret_type, is_pub })
            }
            Some(Token::Return) => {
                self.advance();
                let value = if self.peek() == Some(&Token::Semi) {
                    None
                } else {
                    Some(self.parse_expr(0)?)
                };
                self.consume(Token::Semi, ";")?;
                Ok(Statement::Return(value))
            }
            Some(Token::Let) => {
                self.advance(); // consume 'let'
                let is_mut = if self.peek() == Some(&Token::Mut) {
                    self.advance();
                    true
                } else {
                    false
                };
                
                // Expect identifier
                let name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                
                // Expect '='
                self.consume(Token::Assign, "=")?;
                
                // Expect expression
                let value = self.parse_expr(0)?;
                
                // Expect ';'
                self.consume(Token::Semi, ";")?;
                
                Ok(Statement::Let { name, is_mut, value })
            }
            Some(Token::If) => {
                self.advance(); // consume 'if'
                let old_allowed = self.struct_literal_allowed;
                self.struct_literal_allowed = false;
                let cond = self.parse_expr(0);
                self.struct_literal_allowed = old_allowed;
                let cond = cond?;
                self.consume(Token::LBrace, "{")?;
                let mut then_block = Vec::new();
                while self.peek().is_some() && self.peek() != Some(&Token::RBrace) {
                    then_block.push(self.parse_statement()?);
                }
                self.consume(Token::RBrace, "}")?;
                
                let else_block = if self.peek() == Some(&Token::Else) {
                    self.advance(); // consume 'else'
                    if self.peek() == Some(&Token::If) {
                        Some(vec![self.parse_statement()?])
                    } else {
                        self.consume(Token::LBrace, "{")?;
                        let mut else_stmts = Vec::new();
                        while self.peek().is_some() && self.peek() != Some(&Token::RBrace) {
                            else_stmts.push(self.parse_statement()?);
                        }
                        self.consume(Token::RBrace, "}")?;
                        Some(else_stmts)
                    }
                } else {
                    None
                };
                
                Ok(Statement::If { cond, then_block, else_block })
            }
            Some(Token::While) => {
                self.advance(); // consume 'while'
                let old_allowed = self.struct_literal_allowed;
                self.struct_literal_allowed = false;
                let cond = self.parse_expr(0);
                self.struct_literal_allowed = old_allowed;
                let cond = cond?;
                self.consume(Token::LBrace, "{")?;
                let mut body = Vec::new();
                while self.peek().is_some() && self.peek() != Some(&Token::RBrace) {
                    body.push(self.parse_statement()?);
                }
                self.consume(Token::RBrace, "}")?;
                
                Ok(Statement::While { cond, body })
            }
            Some(Token::Break) => {
                self.advance();
                self.consume(Token::Semi, ";")?;
                Ok(Statement::Break)
            }
            Some(Token::Continue) => {
                self.advance();
                self.consume(Token::Semi, ";")?;
                Ok(Statement::Continue)
            }
            Some(Token::Struct) => {
                self.advance(); // consume 'struct'
                let name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                self.consume(Token::LBrace, "{")?;
                let mut fields = Vec::new();
                if self.peek() != Some(&Token::RBrace) {
                    loop {
                        let field_name = match self.advance() {
                            Some(Token::Ident(name)) => name,
                            found => return Err(ParseError::ExpectedIdentifier { found }),
                        };
                        self.consume(Token::Colon, ":")?;
                        let field_ty = self.parse_type()?;
                        fields.push(ast::FieldDef { name: field_name, ty: field_ty });
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                            if self.peek() == Some(&Token::RBrace) {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                self.consume(Token::RBrace, "}")?;
                Ok(Statement::StructDef { name, fields, is_pub })
            }
            Some(Token::Import) => {
                self.advance(); // consume 'import'
                let mut path = Vec::new();
                let first = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                path.push(first);
                while self.peek() == Some(&Token::ColonColon) {
                    self.advance(); // consume '::'
                    let segment = match self.advance() {
                        Some(Token::Ident(name)) => name,
                        found => return Err(ParseError::ExpectedIdentifier { found }),
                    };
                    path.push(segment);
                }
                self.consume(Token::Semi, ";")?;
                Ok(Statement::Import(path))
            }
            _ => {
                let expr = self.parse_expr(0)?;
                match self.peek() {
                    Some(Token::Assign) => {
                        self.advance(); // consume '='
                        let value = self.parse_expr(0)?;
                        self.consume(Token::Semi, ";")?;
                        match expr {
                            Expr::Ident(name) => Ok(Statement::Assign { name, is_deref: false, value }),
                            Expr::Deref(sub_expr) => {
                                if let Expr::Ident(name) = *sub_expr {
                                    Ok(Statement::Assign { name, is_deref: true, value })
                                } else {
                                    Err(ParseError::InvalidExpression { found: Some(Token::Assign) })
                                }
                            }
                            Expr::FieldAccess { expr: lhs_expr, field } => {
                                Ok(Statement::AssignField { expr: *lhs_expr, field, value })
                            }
                            _ => Err(ParseError::InvalidExpression { found: Some(Token::Assign) })
                        }
                    }
                    Some(tok) if is_assign_op(tok) => {
                        let op_tok = self.advance().unwrap();
                        let op = match op_tok {
                            Token::PlusEq => BinOp::Add,
                            Token::MinusEq => BinOp::Sub,
                            Token::StarEq => BinOp::Mul,
                            Token::SlashEq => BinOp::Div,
                            _ => unreachable!(),
                        };
                        let rhs_expr = self.parse_expr(0)?;
                        self.consume(Token::Semi, ";")?;
                        
                        let desugared_value = Expr::Binary {
                            op,
                            lhs: Box::new(expr.clone()),
                            rhs: Box::new(rhs_expr),
                        };
                        match expr {
                            Expr::Ident(name) => Ok(Statement::Assign { name, is_deref: false, value: desugared_value }),
                            Expr::FieldAccess { expr: lhs_expr, field } => {
                                Ok(Statement::AssignField { expr: *lhs_expr, field, value: desugared_value })
                            }
                            _ => Err(ParseError::InvalidExpression { found: Some(op_tok) })
                        }
                    }
                    _ => {
                        self.consume(Token::Semi, ";")?;
                        Ok(Statement::Expr(expr))
                    }
                }
            }
        }
    }

    fn parse_expr(&mut self, min_precedence: u8) -> Result<Expr<'a>, ParseError<'a>> {
        let mut lhs = self.parse_primary()?;

        while let Some(tok) = self.peek() {
            if tok == &Token::As {
                let precedence = 6;
                if precedence < min_precedence {
                    break;
                }
                self.advance();
                let ty = self.parse_type()?;
                lhs = Expr::As {
                    expr: Box::new(lhs),
                    ty,
                };
                continue;
            }

            if tok == &Token::Is {
                let precedence = 6;
                if precedence < min_precedence {
                    break;
                }
                self.advance();
                let ty = self.parse_type()?;
                lhs = Expr::Is {
                    expr: Box::new(lhs),
                    ty,
                };
                continue;
            }

            let op = match tok {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::EqEq => BinOp::Eq,
                Token::NotEq => BinOp::Ne,
                Token::LessThan => BinOp::Lt,
                Token::LessEq => BinOp::Le,
                Token::GreaterThan => BinOp::Gt,
                Token::GreaterEq => BinOp::Ge,
                Token::AndAnd => BinOp::And,
                Token::OrOr => BinOp::Or,
                _ => break,
            };

            let precedence = op_precedence(op);
            if precedence < min_precedence {
                break;
            }

            self.advance(); // consume operator
            
            // Since all our operators are left-associative, we pass precedence + 1
            let rhs = self.parse_expr(precedence + 1)?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn parse_base_primary(&mut self) -> Result<Expr<'a>, ParseError<'a>> {
        match self.peek() {
            Some(Token::Ident(_)) => {
                let mut segments = Vec::new();
                if let Some(Token::Ident(name)) = self.advance() {
                    segments.push(name);
                }
                while self.peek() == Some(&Token::ColonColon) {
                    self.advance();
                    if let Some(Token::Ident(name)) = self.advance() {
                        segments.push(name);
                    } else {
                        return Err(ParseError::ExpectedIdentifier { found: self.peek().cloned() });
                    }
                }

                let full_name = if segments.len() == 1 {
                    segments[0]
                } else {
                    let joined = segments.join("::");
                    Box::leak(joined.into_boxed_str())
                };

                if self.peek() == Some(&Token::LParen) {
                    self.advance(); // consume '('
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            args.push(self.parse_expr(0)?);
                            if self.peek() == Some(&Token::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.consume(Token::RParen, ")")?;
                    Ok(Expr::Call { name: full_name, args })
                } else if self.struct_literal_allowed && self.peek() == Some(&Token::LBrace) {
                    self.advance(); // consume '{'
                    let mut fields = Vec::new();
                    if self.peek() != Some(&Token::RBrace) {
                        loop {
                            let field_name = match self.advance() {
                                Some(Token::Ident(name)) => name,
                                found => return Err(ParseError::ExpectedIdentifier { found }),
                            };
                            self.consume(Token::Colon, ":")?;
                            let value = self.parse_expr(0)?;
                            fields.push(ast::FieldInit { name: field_name, value });
                            if self.peek() == Some(&Token::Comma) {
                                self.advance();
                                if self.peek() == Some(&Token::RBrace) {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    self.consume(Token::RBrace, "}")?;
                    Ok(Expr::StructLiteral { name: full_name, fields })
                } else {
                    Ok(Expr::Ident(full_name))
                }
            }
            Some(Token::Int(_)) => {
                if let Some(Token::Int(val)) = self.advance() {
                    Ok(Expr::Int(val))
                } else { unreachable!() }
            }
            Some(Token::Float(_)) => {
                if let Some(Token::Float(val)) = self.advance() {
                    Ok(Expr::Float(val))
                } else { unreachable!() }
            }
            Some(Token::Bool(_)) => {
                if let Some(Token::Bool(val)) = self.advance() {
                    Ok(Expr::Bool(val))
                } else { unreachable!() }
            }
            Some(Token::Str(_)) => {
                if let Some(Token::Str(val)) = self.advance() {
                    Ok(Expr::Str(val))
                } else { unreachable!() }
            }
            Some(Token::Bang) => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expr::Unary { op: UnOp::Not, expr: Box::new(expr) })
            }
            Some(Token::Minus) => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expr::Unary { op: UnOp::Neg, expr: Box::new(expr) })
            }
            Some(Token::Ampersand) => {
                self.advance();
                let is_mut = if self.peek() == Some(&Token::Mut) {
                    self.advance();
                    true
                } else {
                    false
                };
                let expr = self.parse_primary()?;
                Ok(Expr::Borrow { is_mut, expr: Box::new(expr) })
            }
            Some(Token::Star) => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expr::Deref(Box::new(expr)))
            }
            Some(Token::Builtin(name)) => {
                let name_val = *name;
                self.advance();
                self.consume(Token::LParen, "(")?;
                let mut args = Vec::new();
                if self.peek() != Some(&Token::RParen) {
                    loop {
                        args.push(self.parse_expr(0)?);
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(Token::RParen, ")")?;
                Ok(Expr::BuiltinCall { name: name_val, args })
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expr(0)?;
                self.consume(Token::RParen, ")")?;
                Ok(expr)
            }
            found => Err(ParseError::InvalidExpression { found: found.cloned() }),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr<'a>, ParseError<'a>> {
        let mut expr = self.parse_base_primary()?;
        while let Some(tok) = self.peek() {
            match tok {
                Token::Dot => {
                    self.advance(); // consume '.'
                    let field = match self.advance() {
                        Some(Token::Ident(field)) => field,
                        found => return Err(ParseError::ExpectedIdentifier { found }),
                    };
                    expr = Expr::FieldAccess {
                        expr: Box::new(expr),
                        field,
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }
}

fn op_precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
        BinOp::Add | BinOp::Sub => 4,
        BinOp::Mul | BinOp::Div => 5,
    }
}

pub fn parse<'a>(tokens: Vec<Token<'a>>) -> Result<Program<'a>, ParseError<'a>> {
    let mut parser = Parser::new(tokens);
    parser.parse()
}

fn is_assign_op(tok: &Token) -> bool {
    matches!(tok, Token::PlusEq | Token::MinusEq | Token::StarEq | Token::SlashEq)
}
