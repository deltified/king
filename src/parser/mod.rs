pub mod ast;

use crate::lexer::Token;
pub use ast::{Program, Statement, Expr, BinOp};

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

    pub fn parse(&mut self) -> Result<Program<'a>, ParseError<'a>> {
        let mut statements = Vec::new();
        while self.peek().is_some() {
            statements.push(self.parse_statement()?);
        }
        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement<'a>, ParseError<'a>> {
        match self.peek() {
            Some(Token::Let) => {
                self.advance(); // consume 'let'
                
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
                
                Ok(Statement::Let { name, value })
            }
            Some(Token::Ident(name)) => {
                // Look ahead to differentiate between assignment and expression
                if self.tokens.get(self.pos + 1) == Some(&Token::Assign) {
                    let name_val = *name;
                    self.advance(); // consume identifier
                    self.advance(); // consume '='
                    
                    let value = self.parse_expr(0)?;
                    self.consume(Token::Semi, ";")?;
                    Ok(Statement::Assign { name: name_val, value })
                } else {
                    let value = self.parse_expr(0)?;
                    self.consume(Token::Semi, ";")?;
                    Ok(Statement::Expr(value))
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
            let op = match tok {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
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
            Some(Token::Ident(name)) => Ok(Expr::Ident(name)),
            Some(Token::Int(val)) => Ok(Expr::Int(val)),
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
        BinOp::Add | BinOp::Sub => 1,
        BinOp::Mul | BinOp::Div => 2,
    }
}

pub fn parse<'a>(tokens: Vec<Token<'a>>) -> Result<Program<'a>, ParseError<'a>> {
    let mut parser = Parser::new(tokens);
    parser.parse()
}
