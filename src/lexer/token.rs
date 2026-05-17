#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {
    Let,
    Fn,
    Return,
    Mut,
    
    Ident(&'a str),
    Int(i64),
    
    Assign, 
    Plus,   
    Minus,  
    Star,  
    Slash, 
    
    LParen, 
    RParen, 
    Semi, 
    
    Colon,
    Comma,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Arrow,
    LessThan,
    GreaterThan,
    Ampersand,
    
    Unknown(char),
}