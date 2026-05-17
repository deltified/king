mod lexer;
mod parser;

use lexer::Lexer;
use lexer::Token;

fn main() {
    test_lexing();
    test_parsing();
}


fn test_lexing() {
    let fn_input = "fn add(x: i64, mut y: i64) -> i64 { return x + y; }";
    let lexer = Lexer::new(fn_input);
    let tokens = lexer.tokenize();
    assert_eq!(
        tokens,
        vec![
            Token::Fn,
            Token::Ident("add"),
            Token::LParen,
            Token::Ident("x"),
            Token::Colon,
            Token::Ident("i64"),
            Token::Comma,
            Token::Mut,
            Token::Ident("y"),
            Token::Colon,
            Token::Ident("i64"),
            Token::RParen,
            Token::Arrow,
            Token::Ident("i64"),
            Token::LBrace,
            Token::Return,
            Token::Ident("x"),
            Token::Plus,
            Token::Ident("y"),
            Token::Semi,
            Token::RBrace,
        ]
    );
    println!("lexer passed test!");
}

fn test_parsing() {

    let fn_input = "fn add(x: i64, y: i64) -> i64 { return x + y; }";
    let lexer = Lexer::new(fn_input);
    let tokens = lexer.tokenize();
    let ast = parser::parse(tokens).expect("Failed to parse function");

    assert_eq!(
        ast,
        Program {
            statements: vec![
                Statement::Function {
                    name: "add",
                    params: vec![
                        Param { name: "x", ty: "i64" },
                        Param { name: "y", ty: "i64" },
                    ],
                    ret_type: Some("i64"),
                    body: vec![
                        Statement::Return(Some(Expr::Binary {
                            op: BinOp::Add,
                            lhs: Box::new(Expr::Ident("x")),
                            rhs: Box::new(Expr::Ident("y")),
                        })),
                    ],
                }
            ]
        }
    );
    println!("parser passed test!");
}

