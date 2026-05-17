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
                let mut is_receiver = false;
                let mut is_amp = false;
                let mut is_mut = false;

                if self.peek() == Some(&Token::Ampersand) {
                    is_receiver = true;
                    is_amp = true;
                    self.advance();
                    if self.peek() == Some(&Token::Mut) {
                        is_mut = true;
                        self.advance();
                    }
                    self.consume(Token::Ident("self"), "self")?;
                } else if self.peek() == Some(&Token::Ident("self")) {
                    is_receiver = true;
                    self.advance();
                }

                let (param_name, param_ty) = if is_receiver {
                    let name = "self";
                    let ty = if self.peek() == Some(&Token::Colon) {
                        self.advance(); // consume ':'
                        let parsed_ty = self.parse_type()?;
                        if is_amp {
                            Type::Ref { is_mut, ty: Box::new(parsed_ty) }
                        } else {
                            parsed_ty
                        }
                    } else {
                        let placeholder = Type::Ident("self");
                        if is_amp {
                            Type::Ref { is_mut, ty: Box::new(placeholder) }
                        } else {
                            placeholder
                        }
                    };
                    (name, ty)
                } else {
                    let _is_param_mut = if self.peek() == Some(&Token::Mut) {
                        self.advance();
                        true
                    } else {
                        false
                    };
                    let param_name = match self.advance() {
                        Some(Token::Ident(name)) => name,
                        Some(Token::Others) => "others",
                        found => return Err(ParseError::ExpectedIdentifier { found }),
                    };
                    self.consume(Token::Colon, ":")?;
                    let param_ty = self.parse_type()?;
                    (param_name, param_ty)
                };

                let mut contract = None;
                if self.peek() == Some(&Token::LBracket) {
                    self.advance(); // consume '['
                    let mut contract_tokens = Vec::new();
                    let mut bracket_depth = 1;
                    while let Some(tok) = self.peek() {
                        if tok == &Token::LBracket {
                            bracket_depth += 1;
                        } else if tok == &Token::RBracket {
                            bracket_depth -= 1;
                            if bracket_depth == 0 {
                                break;
                            }
                        }
                        contract_tokens.push(self.advance().unwrap());
                    }
                    self.consume(Token::RBracket, "]")?;
                    let preprocessed = preprocess_contract_tokens(&contract_tokens, param_name);
                    let mut contract_parser = Parser::new(preprocessed);
                    let contract_expr = contract_parser.parse_expr(0)?;
                    contract = Some(contract_expr);
                }

                let mut default = None;
                if self.peek() == Some(&Token::Assign) {
                    self.advance(); // consume '='
                    default = Some(self.parse_expr(0)?);
                }

                params.push(Param { name: param_name, ty: param_ty, contract, default });
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
                Some(Token::Fn) | Some(Token::Struct) | Some(Token::Extern) | Some(Token::Trait) => {}
                found => return Err(ParseError::UnexpectedToken {
                    expected: "fn, struct, extern or trait after pub",
                    found: found.cloned(),
                }),
            }
        }

        match self.peek() {
            Some(Token::Inline) => {
                self.advance(); // consume 'inline'
                self.consume(Token::For, "for")?;
                let var_name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                self.consume(Token::In, "in")?;
                let start = self.parse_expr(0)?;
                self.consume(Token::DotDot, "..")?;
                let end = self.parse_expr(0)?;
                self.consume(Token::LBrace, "{")?;
                let mut body = Vec::new();
                while self.peek().is_some() && self.peek() != Some(&Token::RBrace) {
                    body.push(self.parse_statement()?);
                }
                self.consume(Token::RBrace, "}")?;
                Ok(Statement::InlineFor { var_name, start, end, body })
            }
            Some(Token::Fn) => {
                self.advance();
                let name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                let mut generics = Vec::new();
                let mut generic_contracts = Vec::new();
                if self.peek() == Some(&Token::LessThan) {
                    self.advance(); // consume '<'
                    while self.peek().is_some() && self.peek() != Some(&Token::GreaterThan) {
                        let gen_name = match self.advance() {
                            Some(Token::Ident(n)) => n,
                            found => return Err(ParseError::ExpectedIdentifier { found }),
                        };
                        
                        let mut contract = None;
                        if self.peek() == Some(&Token::LBracket) {
                            self.advance(); // consume '['
                            let mut contract_tokens = Vec::new();
                            let mut bracket_depth = 1;
                            while let Some(tok) = self.peek() {
                                if tok == &Token::LBracket {
                                    bracket_depth += 1;
                                } else if tok == &Token::RBracket {
                                    bracket_depth -= 1;
                                    if bracket_depth == 0 {
                                        break;
                                    }
                                }
                                contract_tokens.push(self.advance().unwrap());
                            }
                            self.consume(Token::RBracket, "]")?;
                            let preprocessed = preprocess_contract_tokens(&contract_tokens, gen_name);
                            let mut contract_parser = Parser::new(preprocessed);
                            let contract_expr = contract_parser.parse_expr(0)?;
                            contract = Some(contract_expr);
                        }

                        generics.push(gen_name);
                        generic_contracts.push(contract);
                        
                        if self.peek() == Some(&Token::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.consume(Token::GreaterThan, ">")?;
                }
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
                Ok(Statement::Function { name, generics, generic_contracts, params, ret_type, body, is_pub })
            }
            Some(Token::Comptime) => {
                self.advance(); // consume 'comptime'
                self.consume(Token::LBrace, "{")?;
                let mut body = Vec::new();
                while self.peek().is_some() && self.peek() != Some(&Token::RBrace) {
                    body.push(self.parse_statement()?);
                }
                self.consume(Token::RBrace, "}")?;
                Ok(Statement::Comptime(body))
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
            Some(Token::Trait) => {
                self.advance(); // consume 'trait'
                let name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                self.consume(Token::LBrace, "{")?;
                let mut methods = Vec::new();
                while self.peek().is_some() && self.peek() != Some(&Token::RBrace) {
                    self.consume(Token::Fn, "fn")?;
                    let method_name = match self.advance() {
                        Some(Token::Ident(n)) => n,
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
                    methods.push(ast::TraitMethod { name: method_name, params, ret_type });
                }
                self.consume(Token::RBrace, "}")?;
                Ok(Statement::TraitDef { name, methods, is_pub })
            }
            Some(Token::Impl) => {
                self.advance(); // consume 'impl'
                let trait_name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    found => return Err(ParseError::ExpectedIdentifier { found }),
                };
                self.consume(Token::For, "for")?;
                let mut for_types = Vec::new();
                loop {
                    for_types.push(self.parse_type()?);
                    if self.peek() == Some(&Token::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.consume(Token::LBrace, "{")?;
                let mut methods = Vec::new();
                while self.peek().is_some() && self.peek() != Some(&Token::RBrace) {
                    let method = self.parse_statement()?;
                    match &method {
                        Statement::Function { .. } => {
                            methods.push(method);
                        }
                        _ => return Err(ParseError::UnexpectedToken {
                            expected: "method definition inside impl",
                            found: self.peek().cloned(),
                        }),
                    }
                }
                self.consume(Token::RBrace, "}")?;
                Ok(Statement::ImplDef { trait_name, for_types, methods })
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
            Some(Token::Ident(_)) | Some(Token::Others) => {
                let mut segments = Vec::new();
                if let Some(tok) = self.advance() {
                    match tok {
                        Token::Ident(name) => segments.push(name),
                        Token::Others => segments.push("others"),
                        _ => unreachable!(),
                    }
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

                let mut type_args = Vec::new();
                if self.peek() == Some(&Token::LessThan) {
                    let mut temp_pos = self.pos + 1;
                    let mut depth = 1;
                    let mut found_call = false;
                    while temp_pos < self.tokens.len() {
                        match &self.tokens[temp_pos] {
                            Token::LessThan => depth += 1,
                            Token::GreaterThan => {
                                depth -= 1;
                                if depth == 0 {
                                    if temp_pos + 1 < self.tokens.len() && self.tokens[temp_pos + 1] == Token::LParen {
                                        found_call = true;
                                    }
                                    break;
                                }
                            }
                            _ => {}
                        }
                        temp_pos += 1;
                    }

                    if found_call {
                        self.advance(); // consume '<'
                        while self.peek().is_some() && self.peek() != Some(&Token::GreaterThan) {
                            type_args.push(self.parse_type()?);
                            if self.peek() == Some(&Token::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        self.consume(Token::GreaterThan, ">")?;
                    }
                }

                if self.peek() == Some(&Token::LParen) {
                    self.advance(); // consume '('
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            let arg_expr = self.parse_expr(0)?;
                            let mut arg_name = None;
                            let mut arg_val = arg_expr;
                            if let Expr::Ident(id_name) = &arg_val {
                                if self.peek() == Some(&Token::Colon) {
                                    self.advance(); // consume ':'
                                    arg_name = Some(*id_name);
                                    arg_val = self.parse_expr(0)?;
                                }
                            }
                            args.push(ast::CallArg { name: arg_name, value: arg_val });
                            if self.peek() == Some(&Token::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.consume(Token::RParen, ")")?;
                    Ok(Expr::Call { name: full_name, type_args, args })
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
                    if self.peek() == Some(&Token::LParen) {
                        self.advance(); // consume '('
                        let mut args = Vec::new();
                        if self.peek() != Some(&Token::RParen) {
                            loop {
                                let arg_expr = self.parse_expr(0)?;
                                let mut arg_name = None;
                                let mut arg_val = arg_expr;
                                if let Expr::Ident(id_name) = &arg_val {
                                    if self.peek() == Some(&Token::Colon) {
                                        self.advance(); // consume ':'
                                        arg_name = Some(*id_name);
                                        arg_val = self.parse_expr(0)?;
                                    }
                                }
                                args.push(ast::CallArg { name: arg_name, value: arg_val });
                                if self.peek() == Some(&Token::Comma) {
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                        }
                        self.consume(Token::RParen, ")")?;
                        expr = Expr::MethodCall {
                            expr: Box::new(expr),
                            method: field,
                            args,
                        };
                    } else {
                        expr = Expr::FieldAccess {
                            expr: Box::new(expr),
                            field,
                        };
                    }
                }
                Token::LBracket => {
                    self.advance(); // consume '['
                    let index = self.parse_expr(0)?;
                    self.consume(Token::RBracket, "]")?;
                    expr = Expr::IndexAccess {
                        expr: Box::new(expr),
                        index: Box::new(index),
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

fn is_lhs_missing_eligible(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Slash
            | Token::EqEq
            | Token::NotEq
            | Token::LessThan
            | Token::LessEq
            | Token::GreaterThan
            | Token::GreaterEq
            | Token::Is
            | Token::As
    )
}

fn preprocess_contract_tokens<'a>(tokens: &[Token<'a>], name: &'a str) -> Vec<Token<'a>> {
    let mut result = Vec::new();
    for i in 0..tokens.len() {
        let tok = &tokens[i];
        let is_missing = is_lhs_missing_eligible(tok) && {
            if i == 0 {
                true
            } else {
                matches!(
                    &tokens[i - 1],
                    Token::AndAnd | Token::OrOr | Token::LParen | Token::Bang
                )
            }
        };
        if is_missing {
            result.push(Token::Ident(name));
        }
        result.push(tok.clone());
    }
    result
}
