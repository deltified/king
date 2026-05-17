mod lexer;
mod parser;
mod hir;
mod sema;
mod mir;
mod codegen;
mod analysis;

fn load_all_files(
    file_path: &std::path::Path,
    module_name: &str,
    loaded_files: &mut std::collections::HashMap<std::path::PathBuf, String>,
    compilation_stack: &mut Vec<std::path::PathBuf>,
    modules: &mut Vec<(String, parser::Program<'static>, Vec<Vec<String>>)>,
) -> Result<(), String> {
    let canonical = file_path.canonicalize().map_err(|e| format!("Failed to resolve path {:?}: {}", file_path, e))?;
    
    if compilation_stack.contains(&canonical) {
        return Err(format!("Circular import detected: {:?}", compilation_stack));
    }
    
    if loaded_files.contains_key(&canonical) {
        return Ok(());
    }
    
    let content = std::fs::read_to_string(&canonical).map_err(|e| format!("Failed to read file {:?}: {}", canonical, e))?;
    // We leak the content to get 'static lifetime! This is extremely safe and fast for compilers.
    let content_ref: &'static str = Box::leak(content.clone().into_boxed_str());
    loaded_files.insert(canonical.clone(), content);
    
    compilation_stack.push(canonical.clone());
    
    let lexer = lexer::Lexer::new(content_ref);
    let tokens = lexer.tokenize();
    let ast = parser::parse(tokens).map_err(|e| format!("Parse error in {:?}: {:?}", canonical, e))?;
    
    // Extract imports and filter them from AST statements
    let mut imports = Vec::new();
    let mut clean_statements = Vec::new();
    for stmt in ast.statements {
        if let parser::Statement::Import(path_segments) = stmt {
            let segments: Vec<String> = path_segments.into_iter().map(|s| s.to_string()).collect();
            imports.push(segments);
        } else {
            clean_statements.push(stmt);
        }
    }
    
    // Process imports first to strictly disallow circular imports and load dependencies
    let parent_dir = canonical.parent().unwrap_or_else(|| std::path::Path::new("."));
    for imp in &imports {
        // Resolve import path relative to the importing file
        let mut resolved_path = parent_dir.to_path_buf();
        for segment in imp {
            resolved_path.push(segment);
        }
        resolved_path.set_extension("king");
        
        if !resolved_path.exists() {
            // Fall back to resolving relative to the main file's parent directory (first stack element)
            if let Some(root_file) = compilation_stack.first() {
                if let Some(root_dir) = root_file.parent() {
                    let mut fallback = root_dir.to_path_buf();
                    for segment in imp {
                        fallback.push(segment);
                    }
                    fallback.set_extension("king");
                    if fallback.exists() {
                        resolved_path = fallback;
                    }
                }
            }
        }
        
        let imp_module_name = imp.join("::");
        load_all_files(&resolved_path, &imp_module_name, loaded_files, compilation_stack, modules)?;
    }
    
    compilation_stack.pop();
    
    modules.push((module_name.to_string(), parser::Program { statements: clean_statements }, imports));
    Ok(())
}

fn compile_file(input_path: &str, output_path: &str) -> Result<(), String> {
    let mut loaded_files = std::collections::HashMap::new();
    let mut compilation_stack = Vec::new();
    let mut modules = Vec::new();
    
    load_all_files(
        std::path::Path::new(input_path),
        "main",
        &mut loaded_files,
        &mut compilation_stack,
        &mut modules,
    )?;
    
    let mut hir_structs = Vec::new();
    let mut hir_functions = Vec::new();
    let mut imports_map = std::collections::HashMap::new();
    
    for (mod_name, ast, imports) in modules {
        let hir_prog = hir::build(ast, &mod_name);
        hir_structs.extend(hir_prog.structs);
        hir_functions.extend(hir_prog.functions);
        
        let imported_names: Vec<String> = imports.into_iter().map(|imp| imp.join("::")).collect();
        imports_map.insert(mod_name, imported_names);
    }
    
    let hir_prog = hir::Program {
        structs: hir_structs,
        functions: hir_functions,
        imports: imports_map,
    };
    
    let typed_hir = sema::analyze(hir_prog)?;
    let mir_prog = mir::build(typed_hir);
    analysis::check_program(&mir_prog)?;
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
        println!("  PATH=\"/opt/homebrew/opt/llvm/bin:$PATH\" cargo test -- --nocapture");
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
            ("tests/reference.king", 42),
            ("tests/struct_simple.king", 42),
            ("tests/struct_mutability.king", 42),
            ("tests/import_success.king", 60),
            ("tests/strings.king", 42),
        ];
        let mut failed = Vec::new();
        let mut passed = Vec::new();
        for (source_file, expected_ret) in test_cases {
            let pid = std::process::id();
            let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let temp_dir = std::env::temp_dir();
            let ir_path = temp_dir.join(format!("output_{}_{}.ll", pid, counter));
            let output_bin_path = temp_dir.join(format!("output_bin_{}_{}", pid, counter));
            let _cleanup = TempCleanup {
                paths: vec![ir_path.clone(), output_bin_path.clone()],
            };
            if let Err(e) = compile_file(source_file, ir_path.to_str().unwrap()) {
                failed.push((source_file, format!("Compilation failed: {}", e)));
                continue;
            }
            let compile_status = Command::new("clang")
                .arg(&ir_path)
                .arg("-o")
                .arg(&output_bin_path)
                .status();
            let status = match compile_status {
                Ok(s) => s,
                Err(e) => {
                    failed.push((source_file, format!("Failed to run clang: {}", e)));
                    continue;
                }
            };
            if !status.success() {
                failed.push((source_file, "Clang compilation failed".to_string()));
                continue;
            }
            let run_output = Command::new(&output_bin_path).status();
            let run_status = match run_output {
                Ok(s) => s,
                Err(e) => {
                    failed.push((source_file, format!("Failed to run executable: {}", e)));
                    continue;
                }
            };
            let exit_code = match run_status.code() {
                Some(code) => code,
                None => {
                    failed.push((source_file, "Process terminated by signal".to_string()));
                    continue;
                }
            };
            if exit_code == expected_ret {
                passed.push(source_file);
            } else {
                failed.push((source_file, format!("Expected exit code {}, got {}", expected_ret, exit_code)));
            }
        }
        for test in &passed {
            println!("Test PASSED: {}", test);
        }
        for (test, err) in &failed {
            println!("Test FAILED: {} ({})", test, err);
        }
        assert!(failed.is_empty(), "Some tests failed!");
    }

    #[test]
    fn test_compiler_errors() {
        let error_test_cases = vec![
            ("tests/borrow_err_double_mut.king", "already borrowed"),
            ("tests/borrow_err_mut_and_immut.king", "already borrowed mutably"),
            ("tests/borrow_err_write_borrowed.king", "borrowed"),
            ("tests/borrow_err_read_mut_borrowed.king", "mutably borrowed"),
            ("tests/borrow_err_write_immut_ref.king", "immutable reference"),
            ("tests/borrow_err_return_local_ref.king", "Cannot return reference to local variable"),
            ("tests/borrow_err_struct_double_mut.king", "already borrowed"),
            ("tests/borrow_err_struct_write_borrowed.king", "borrowed"),
            ("tests/import_err_private.king", "private"),
            ("tests/import_err_circular.king", "Circular import detected"),
        ];

        let temp_dir = std::env::temp_dir();
        for (source_file, expected_err) in error_test_cases {
            let pid = std::process::id();
            let ir_path = temp_dir.join(format!("output_err_{}.ll", pid));
            let _cleanup = TempCleanup {
                paths: vec![ir_path.clone()],
            };

            let res = compile_file(source_file, ir_path.to_str().unwrap());
            assert!(res.is_err(), "Expected {} to fail compilation, but it succeeded!", source_file);
            let err_msg = res.unwrap_err();
            assert!(
                err_msg.contains(expected_err),
                "Expected error for {} to contain '{}', but got '{}'",
                source_file, expected_err, err_msg
            );
            println!("Error Test PASSED: {} (Failed as expected with: {})", source_file, err_msg);
        }
    }
}