mod token;
pub use token::Token;
// Lifetime annotations for "expandability" or so they said

pub struct Lexer<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    pub fn tokenize(mut self) -> Vec<Token<'a>> {
        let mut tokens = Vec::new();
        while let Some(tok) = self.next_token() {
            tokens.push(tok);
        }
        tokens
    }

    fn next_token(&mut self) -> Option<Token<'a>> {
        self.skip_whitespace();
        
        if self.pos >= self.bytes.len() {
            return None;
        }

        let b = self.bytes[self.pos];
        let token = match b {
            b'=' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'=' {
                    self.pos += 2;
                    Token::EqEq
                } else {
                    self.pos += 1;
                    Token::Assign
                }
            }
            b'+' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'=' {
                    self.pos += 2;
                    Token::PlusEq
                } else {
                    self.pos += 1;
                    Token::Plus
                }
            }
            b'-' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'>' {
                    self.pos += 2;
                    Token::Arrow
                } else if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'=' {
                    self.pos += 2;
                    Token::MinusEq
                } else {
                    self.pos += 1;
                    Token::Minus
                }
            }
            b'*' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'=' {
                    self.pos += 2;
                    Token::StarEq
                } else {
                    self.pos += 1;
                    Token::Star
                }
            }
            b'/' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'=' {
                    self.pos += 2;
                    Token::SlashEq
                } else {
                    self.pos += 1;
                    Token::Slash
                }
            }
            b'(' => { self.pos += 1; Token::LParen }
            b')' => { self.pos += 1; Token::RParen }
            b';' => { self.pos += 1; Token::Semi }
            b':' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b':' {
                    self.pos += 2;
                    Token::ColonColon
                } else {
                    self.pos += 1;
                    Token::Colon
                }
            }
            b',' => { self.pos += 1; Token::Comma }
            b'{' => { self.pos += 1; Token::LBrace }
            b'}' => { self.pos += 1; Token::RBrace }
            b'[' => { self.pos += 1; Token::LBracket }
            b']' => { self.pos += 1; Token::RBracket }
            b'.' => { self.pos += 1; Token::Dot }
            b'<' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'=' {
                    self.pos += 2;
                    Token::LessEq
                } else {
                    self.pos += 1;
                    Token::LessThan
                }
            }
            b'>' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'=' {
                    self.pos += 2;
                    Token::GreaterEq
                } else {
                    self.pos += 1;
                    Token::GreaterThan
                }
            }
            b'&' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'&' {
                    self.pos += 2;
                    Token::AndAnd
                } else {
                    self.pos += 1;
                    Token::Ampersand
                }
            }
            b'|' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'|' {
                    self.pos += 2;
                    Token::OrOr
                } else {
                    self.pos += 1;
                    Token::Unknown('|')
                }
            }
            b'!' => {
                if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'=' {
                    self.pos += 2;
                    Token::NotEq
                } else {
                    self.pos += 1;
                    Token::Bang
                }
            }
            
            // Fast paths for Identifiers and Numbers
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => return Some(self.read_identifier()),
            b'0'..=b'9' => return Some(self.read_number()),
            
            // Safe and fancy UTF-8 fallback 
            _ => {
                let c = self.input[self.pos..].chars().next().unwrap();
                self.pos += c.len_utf8();
                Token::Unknown(c)
            }
        };
        
        Some(token)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn read_identifier(&mut self) -> Token<'a> {
        let start = self.pos;
        while self.pos < self.bytes.len() 
            && (self.bytes[self.pos].is_ascii_alphanumeric() || self.bytes[self.pos] == b'_') 
        {
            self.pos += 1;
        }
        
        let text = &self.input[start..self.pos];
        match text {
            "let" => Token::Let,
            "fn" => Token::Fn,
            "return" => Token::Return,
            "mut" => Token::Mut,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "true" => Token::Bool(true),
            "false" => Token::Bool(false),
            "as" => Token::As,
            "break" => Token::Break,
            "continue" => Token::Continue,
            "struct" => Token::Struct,
            "import" => Token::Import,
            "pub" => Token::Pub,
            _ => Token::Ident(text),
        }
    }

    fn read_number(&mut self) -> Token<'a> {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b'.' {
            if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1].is_ascii_digit() {
                self.pos += 1; // consume '.'
                while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
                let val: f64 = self.input[start..self.pos].parse().unwrap();
                return Token::Float(val);
            }
        }
        
        let val: i64 = self.input[start..self.pos].parse().unwrap();
        Token::Int(val)
    }
}