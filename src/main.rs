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

    // 7. Write LLVM IR to file and compile & run with clang
    let ir_path = std::path::Path::new("output.ll");
    module.print_to_file(ir_path).expect("Failed to write LLVM IR to file");

    let main_c = r#"
#include <stdio.h>

extern long long compute(long long x);

int main() {
    long long result = compute(10);
    printf("compute(10) = %lld\n", result);
    return 0;
}
"#;
    std::fs::write("main.c", main_c).expect("Failed to write main.c");

    println!("Compiling output.ll and main.c with clang...");
    let compile_status = std::process::Command::new("clang")
        .arg("output.ll")
        .arg("main.c")
        .arg("-o")
        .arg("output_bin")
        .status()
        .expect("Failed to execute clang");

    if !compile_status.success() {
        panic!("Clang compilation failed");
    }

    println!("Running output_bin...");
    let run_output = std::process::Command::new("./output_bin")
        .output()
        .expect("Failed to run output_bin");

    if !run_output.status.success() {
        panic!("Executable run failed");
    }

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    println!("Output of compiled program:\n{}", stdout);

    // Clean up temporary files
    let _ = std::fs::remove_file("output.ll");
    let _ = std::fs::remove_file("main.c");
    let _ = std::fs::remove_file("output_bin");
}

