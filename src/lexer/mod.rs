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
            b'=' => { self.pos += 1; Token::Assign }
            b'+' => { self.pos += 1; Token::Plus }
            b'-' => { self.pos += 1; Token::Minus }
            b'*' => { self.pos += 1; Token::Star }
            b'/' => { self.pos += 1; Token::Slash }
            b'(' => { self.pos += 1; Token::LParen }
            b')' => { self.pos += 1; Token::RParen }
            b';' => { self.pos += 1; Token::Semi }
            
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
            _ => Token::Ident(text),
        }
    }

    fn read_number(&mut self) -> Token<'a> {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        
        // This unwrap should never actually panic 
        let val: i64 = self.input[start..self.pos].parse().unwrap();
        Token::Int(val)
    }
}