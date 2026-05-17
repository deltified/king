#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {
    Let,
    Fn,
    Return,
    Mut,
    
    If,
    Else,
    While,
    Bool(bool),
    
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
    
    AndAnd,
    OrOr,
    Bang,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    EqEq,
    NotEq,
    LessEq,
    GreaterEq,
    
    Unknown(char),
}