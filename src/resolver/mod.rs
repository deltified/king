use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::lexer;
use crate::parser;

pub struct ModuleData {
    pub name: String,
    pub ast: parser::Program<'static>,
    pub imports: Vec<Vec<String>>,
}

pub struct Resolver {
    pub loaded_files: HashMap<PathBuf, String>,
    pub modules: Vec<ModuleData>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            loaded_files: HashMap::new(),
            modules: Vec::new(),
        }
    }

    pub fn resolve(&mut self, start_path: &Path) -> Result<(), String> {
        let mut compilation_stack = Vec::new();
        self.load_all_files(start_path, "main", &mut compilation_stack)
    }

    fn load_all_files(
        &mut self,
        file_path: &Path,
        module_name: &str,
        compilation_stack: &mut Vec<PathBuf>,
    ) -> Result<(), String> {
        let canonical = file_path.canonicalize().map_err(|e| format!("Failed to resolve path {:?}: {}", file_path, e))?;
        
        if compilation_stack.contains(&canonical) {
            return Err(format!("Circular import detected: {:?}", compilation_stack));
        }
        
        if self.loaded_files.contains_key(&canonical) {
            return Ok(());
        }
        
        let content = std::fs::read_to_string(&canonical).map_err(|e| format!("Failed to read file {:?}: {}", canonical, e))?;
        // We leak the content to get 'static lifetime! This is extremely safe and fast for compilers.
        let content_ref: &'static str = Box::leak(content.clone().into_boxed_str());
        self.loaded_files.insert(canonical.clone(), content);
        
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
        let parent_dir = canonical.parent().unwrap_or_else(|| Path::new("."));
        for imp in &imports {
            let mut resolved_path = PathBuf::new();
            
            if imp.first().map(|s| s.as_str()) == Some("std") {
                // Resolve std:: relative to the current working directory (project root)
                if let Ok(cwd) = std::env::current_dir() {
                    resolved_path = cwd.to_path_buf();
                    for segment in imp {
                        resolved_path.push(segment);
                    }
                }
            } else {
                // Resolve import path relative to the importing file
                resolved_path = parent_dir.to_path_buf();
                for segment in imp {
                    resolved_path.push(segment);
                }
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
            self.load_all_files(&resolved_path, &imp_module_name, compilation_stack)?;
        }
        
        compilation_stack.pop();
        
        self.modules.push(ModuleData {
            name: module_name.to_string(),
            ast: parser::Program { statements: clean_statements },
            imports,
        });
        Ok(())
    }
}
