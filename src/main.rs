mod lexer;
mod parser;

use lexer::Lexer;
use lexer::Token;
use parser::{Program, Statement, Param, Expr, BinOp};

fn main() {
    test_lexing();
    test_parsing();
    test_llvm();
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

    let complex_input = "fn test() {
        let mut x = 10;
        x += 5;
        if x > 15 {
            x = 0;
        } else {
            while x < 15 {
                x += 1;
            }
        }
    }";
    let lexer = Lexer::new(complex_input);
    let tokens = lexer.tokenize();
    let ast = parser::parse(tokens).expect("Failed to parse complex function");
    assert_eq!(ast.statements.len(), 1);
    println!("parser passed test!");
}

fn test_llvm() {
    use inkwell::context::Context;

    let context = Context::create();
    let module = context.create_module("sum");
    let builder = context.create_builder();

    let i64_type = context.i64_type();
    let fn_type = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);
    let function = module.add_function("sum", fn_type, None);

    let basic_block = context.append_basic_block(function, "entry");
    builder.position_at_end(basic_block);

    let x = function.get_nth_param(0).unwrap().into_int_value();
    let y = function.get_nth_param(1).unwrap().into_int_value();

    let sum = builder.build_int_add(x, y, "sum_val").unwrap();
    builder.build_return(Some(&sum)).unwrap();

    println!("Generated LLVM IR:");
    module.print_to_stderr();
}

