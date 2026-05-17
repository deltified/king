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
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self { tokens, pos: 0 }
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

    pub fn parse(&mut self) -> Result<Program<'a>, ParseError<'a>> {
        let mut statements = Vec::new();
        while self.peek().is_some() {
            statements.push(self.parse_statement()?);
        }
        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement<'a>, ParseError<'a>> {
        match self.peek() {
            Some(Token::Fn) => {
                self.advance();
                let name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
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
                Ok(Statement::Function { name, params, ret_type, body })
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
                let cond = self.parse_expr(0)?;
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
                let cond = self.parse_expr(0)?;
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
            Some(Token::Ident(name)) => {
                let next = self.tokens.get(self.pos + 1);
                match next {
                    Some(Token::Assign) => {
                        let name_val = *name;
                        self.advance(); // consume identifier
                        self.advance(); // consume '='
                        
                        let value = self.parse_expr(0)?;
                        self.consume(Token::Semi, ";")?;
                        Ok(Statement::Assign { name: name_val, value })
                    }
                    Some(Token::PlusEq) | Some(Token::MinusEq) | Some(Token::StarEq) | Some(Token::SlashEq) => {
                        let name_val = *name;
                        self.advance(); // consume identifier
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
                            lhs: Box::new(Expr::Ident(name_val)),
                            rhs: Box::new(rhs_expr),
                        };
                        Ok(Statement::Assign { name: name_val, value: desugared_value })
                    }
                    _ => {
                        let value = self.parse_expr(0)?;
                        self.consume(Token::Semi, ";")?;
                        Ok(Statement::Expr(value))
                    }
                }
            }
            _ => {
                let value = self.parse_expr(0)?;
                self.consume(Token::Semi, ";")?;
                Ok(Statement::Expr(value))
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

    fn parse_primary(&mut self) -> Result<Expr<'a>, ParseError<'a>> {
        match self.advance() {
            Some(Token::Ident(name)) => {
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
                    Ok(Expr::Call { name, args })
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Some(Token::Int(val)) => Ok(Expr::Int(val)),
            Some(Token::Float(val)) => Ok(Expr::Float(val)),
            Some(Token::Bool(val)) => Ok(Expr::Bool(val)),
            Some(Token::Bang) => {
                let expr = self.parse_primary()?;
                Ok(Expr::Unary { op: UnOp::Not, expr: Box::new(expr) })
            }
            Some(Token::Minus) => {
                let expr = self.parse_primary()?;
                Ok(Expr::Unary { op: UnOp::Neg, expr: Box::new(expr) })
            }
            Some(Token::Ampersand) => {
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
                let expr = self.parse_primary()?;
                Ok(Expr::Deref(Box::new(expr)))
            }
            Some(Token::LParen) => {
                let expr = self.parse_expr(0)?;
                self.consume(Token::RParen, ")")?;
                Ok(expr)
            }
            found => Err(ParseError::InvalidExpression { found }),
        }
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
