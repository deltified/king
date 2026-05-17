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
    use lexer::Lexer;
    use parser::{Program, Statement, Param, Expr, BinOp};
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
    fn test_king_source_files() {
        let test_cases = vec![
            ("tests/simple.king", 42),
            ("tests/arithmetic.king", 50),
            ("tests/loop.king", 55),
            ("tests/fibonacci.king", 21),
            ("tests/break_continue.king", 50),
            ("tests/casts.king", 7),
        ];
        for (source_file, expected_ret) in test_cases {
            let pid = std::process::id();
            let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let temp_dir = std::env::temp_dir();
            let ir_path = temp_dir.join(format!("output_{}_{}.ll", pid, counter));
            let output_bin_path = temp_dir.join(format!("output_bin_{}_{}", pid, counter));
            let _cleanup = TempCleanup {
                paths: vec![ir_path.clone(), output_bin_path.clone()],
            };
            compile_file(source_file, ir_path.to_str().unwrap()).expect("Compilation failed");
            let compile_status = Command::new("clang")
                .arg(&ir_path)
                .arg("-o")
                .arg(&output_bin_path)
                .status()
                .expect("Failed to execute clang");
            assert!(compile_status.success(), "Clang compilation failed");
            let run_output = Command::new(&output_bin_path)
                .status()
                .expect("Failed to run output binary");
            let exit_code = run_output.code().expect("Process terminated by signal");
            assert_eq!(exit_code, expected_ret);
        }
    }
}