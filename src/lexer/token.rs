#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {
    Let,
    
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
    
    Unknown(char),
}