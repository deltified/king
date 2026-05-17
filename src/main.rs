mod lexer;
mod parser;
mod hir;
mod sema;
mod mir;
mod codegen;

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
    let input = "fn compute(mut x: i64) -> i64 {
        let mut y = 0;
        while x > 0 {
            if x == 5 {
                y += 100;
            } else {
                y += x;
            }
            x -= 1;
        }
        return y;
    }";
    
    let lexer = Lexer::new(input);
    let tokens = lexer.tokenize();
    
    let ast = parser::parse(tokens).expect("Failed to parse");
    
    let hir_prog = hir::build(ast);
    
    let typed_hir = sema::analyze(hir_prog).expect("Semantic analysis failed");
    
    let mir_prog = mir::build(typed_hir);
    
    use inkwell::context::Context;
    let context = Context::create();
    let codegen = codegen::Codegen::new(&context, "king_module");
    let module = codegen.compile_program(mir_prog);
    
    println!("Generated LLVM IR for end-to-end compiler pipeline:");
    module.print_to_stderr();
}

