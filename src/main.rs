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

    let fn_generic = "fn run<T>(a: T [T]) {}";
    let lexer = Lexer::new(fn_generic);
    let tokens = lexer.tokenize();
    assert_eq!(
        tokens,
        vec![
            Token::Fn,
            Token::Ident("run"),
            Token::LessThan,
            Token::Ident("T"),
            Token::GreaterThan,
            Token::LParen,
            Token::Ident("a"),
            Token::Colon,
            Token::Ident("T"),
            Token::LBracket,
            Token::Ident("T"),
            Token::RBracket,
            Token::RParen,
            Token::LBrace,
            Token::RBrace,
        ]
    );
    println!("lexer passed generic/contract function test!");
}

fn test_parsing() {
    let input = "let result = (42 + 8) * result_v2 / 2;";
    let lexer = Lexer::new(input);
    let tokens = lexer.tokenize();

    let ast = parser::parse(tokens).expect("Failed to parse tokens");

    use parser::{Program, Statement, Expr, BinOp};

    assert_eq!(
        ast,
        Program {
            statements: vec![
                Statement::Let {
                    name: "result",
                    value: Expr::Binary {
                        op: BinOp::Div,
                        lhs: Box::new(Expr::Binary {
                            op: BinOp::Mul,
                            lhs: Box::new(Expr::Binary {
                                op: BinOp::Add,
                                lhs: Box::new(Expr::Int(42)),
                                rhs: Box::new(Expr::Int(8)),
                            }),
                            rhs: Box::new(Expr::Ident("result_v2")),
                        }),
                        rhs: Box::new(Expr::Int(2)),
                    }
                }
            ]
        }
    );
    println!("parser passed test!");
}

