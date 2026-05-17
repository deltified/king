mod lexer;
mod parser;
mod hir;
mod sema;
mod mir;
mod codegen;

fn compile_file(input_path: &str, output_path: &str) -> Result<(), String> {
    let input = std::fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let lexer = lexer::Lexer::new(&input);
    let tokens = lexer.tokenize();
    let ast = parser::parse(tokens).map_err(|e| format!("{:?}", e))?;
    let hir_prog = hir::build(ast);
    let typed_hir = sema::analyze(hir_prog)?;
    let mir_prog = mir::build(typed_hir);
    let context = inkwell::context::Context::create();
    let codegen = codegen::Codegen::new(&context, "king_module");
    let module = codegen.compile_program(mir_prog);
    module.print_to_file(output_path).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("King Compiler");
        println!("To run the compiler test suite, execute:");
        println!("  PATH=\"/opt/homebrew/opt/llvm/bin:$PATH\" cargo test");
        return;
    }
    let mut input_path = None;
    let mut output_path = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                if i + 1 < args.len() {
                    output_path = Some(&args[i + 1]);
                    i += 2;
                } else {
                    std::process::exit(1);
                }
            }
            path => {
                input_path = Some(path);
                i += 1;
            }
        }
    }
    let Some(input) = input_path else {
        std::process::exit(1);
    };
    let default_output = format!("{}.ll", input);
    let output = output_path.map(|s| s.as_str()).unwrap_or(&default_output);
    if compile_file(input, output).is_err() {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lexer::{Lexer, Token};
    use parser::{Program, Statement, Param, Expr, BinOp};
    use inkwell::context::Context;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    // RAII helper to ensure all temporary files are deleted when the test finishes or panics.
    struct TempCleanup {
        paths: Vec<PathBuf>,
    }

    impl Drop for TempCleanup {
        fn drop(&mut self) {
            for path in &self.paths {
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }


    #[test]
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
    }

    #[test]
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
        
        let context = Context::create();
        let codegen = codegen::Codegen::new(&context, "king_module");
        let module = codegen.compile_program(mir_prog);
        
        // Generate unique file names using PID and an atomic counter to prevent conflicts during parallel test runs.
        let pid = std::process::id();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir();

        let ir_path = temp_dir.join(format!("output_{}_{}.ll", pid, counter));
        let main_c_path = temp_dir.join(format!("main_{}_{}.c", pid, counter));
        let output_bin_path = temp_dir.join(format!("output_bin_{}_{}", pid, counter));

        // Create the cleanup guard
        let _cleanup = TempCleanup {
            paths: vec![ir_path.clone(), main_c_path.clone(), output_bin_path.clone()],
        };

        // Write the LLVM IR
        module.print_to_file(&ir_path).expect("Failed to write LLVM IR to file");

        // Write standard C driver code
        let main_c = r#"
#include <stdio.h>

extern long long compute(long long x);

int main() {
    long long result = compute(10);
    printf("compute(10) = %lld\n", result);
    return 0;
}
"#;
        std::fs::write(&main_c_path, main_c).expect("Failed to write main.c");

        // Compile output.ll and main.c with clang
        let compile_status = Command::new("clang")
            .arg(&ir_path)
            .arg(&main_c_path)
            .arg("-o")
            .arg(&output_bin_path)
            .status()
            .expect("Failed to execute clang");

        assert!(compile_status.success(), "Clang compilation failed");

        // Run the compiled executable
        let run_output = Command::new(&output_bin_path)
            .output()
            .expect("Failed to run output binary");

        assert!(run_output.status.success(), "Executable run failed");

        let stdout = String::from_utf8_lossy(&run_output.stdout);
        assert_eq!(stdout.trim(), "compute(10) = 150");
    }
    