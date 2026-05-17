mod lexer;
mod parser;

use lexer::Lexer;
use lexer::Token;

fn main() {
    test_lexing();
    test_parsing();
}


fn test_lexing() {
    let input = "let result = (42 + 8) * result_v2 / 2;";
    let lexer = Lexer::new(input);
    let tokens = lexer.tokenize();

    assert_eq!(
        tokens,
        vec![
            Token::Let,
            Token::Ident("result"),
            Token::Assign,
            Token::LParen,
            Token::Int(42),
            Token::Plus,
            Token::Int(8),
            Token::RParen,
            Token::Star,
            Token::Ident("result_v2"),
            Token::Slash,
            Token::Int(2),
            Token::Semi,
        ]
    );
    println!("lexer passed test!");
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

