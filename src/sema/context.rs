use std::collections::HashMap;
use super::ast::{Type, Function};

pub struct StructMeta<'a> {
    pub original_name: &'a str,
    pub module_name: String,
    pub is_pub: bool,
    pub fields: Vec<(String, Type)>,
}

pub struct FunctionMeta<'a> {
    pub original_name: &'a str,
    pub module_name: String,
    pub is_pub: bool,
    pub is_extern: bool,
    pub param_types: Vec<Type>,
    pub ret_type: Type,
    pub param_names: Vec<String>,
    pub param_defaults: Vec<Option<crate::hir::Expr<'a>>>,
}

pub fn mangle_name(module_name: &str, name: &str, is_extern: bool) -> &'static str {
    if is_extern || module_name == "extern" || name == "main" {
        if name == "main" {
            "main"
        } else {
            Box::leak(name.to_string().into_boxed_str())
        }
    } else {
        let mangled = format!("{}__{}", module_name.replace("::", "_"), name);
        Box::leak(mangled.into_boxed_str())
    }
}

pub struct SemaContext<'a> {
    pub(super) scopes: Vec<HashMap<&'a str, (Type, bool)>>,
    pub(super) functions: HashMap<&'static str, (Vec<Type>, Type)>,
    pub structs: HashMap<String, Vec<(String, Type)>>,
    pub(super) current_ret_type: Option<Type>,
    pub(super) loop_depth: usize,
    pub imports: HashMap<String, Vec<String>>,
    pub all_structs: Vec<StructMeta<'a>>,
    pub all_functions: Vec<FunctionMeta<'a>>,
    pub current_module: String,
    pub generic_templates: HashMap<String, crate::hir::Function<'a>>,
    pub monomorphized_functions: Vec<Function<'a>>,
}

impl<'a> SemaContext<'a> {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            structs: HashMap::new(),
            current_ret_type: None,
            loop_depth: 0,
            imports: HashMap::new(),
            all_structs: Vec::new(),
            all_functions: Vec::new(),
            current_module: String::new(),
            generic_templates: HashMap::new(),
            monomorphized_functions: Vec::new(),
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn declare_var(&mut self, name: &'a str, ty: Type, is_mut: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, (ty, is_mut));
        }
    }

    pub fn lookup_var(&self, name: &str) -> Option<(Type, bool)> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info.clone());
            }
        }
        None
    }

    pub fn resolve_type(&self, ty: Type) -> Result<Type, String> {
        match ty {
            Type::Ref { is_mut, ty } => {
                let resolved_inner = self.resolve_type(*ty)?;
                Ok(Type::Ref {
                    is_mut,
                    ty: Box::new(resolved_inner),
                })
            }
            Type::Struct(name) => self.resolve_struct_type(&name),
            other => Ok(other),
        }
    }

    pub fn resolve_struct_type(&self, name: &str) -> Result<Type, String> {
        if name == "i64"
            || name == "f64"
            || name == "bool"
            || name == "void"
            || name == "char"
            || name == "str"
            || name == "type"
        {
            return Ok(match name {
                "i64" => Type::I64,
                "f64" => Type::F64,
                "bool" => Type::Bool,
                "char" => Type::Char,
                "str" => Type::Str,
                "type" => Type::TypeVal,
                _ => Type::Void,
            });
        }

        if let Some(pos) = name.rfind("::") {
            let mod_name = &name[..pos];
            let struct_name = &name[pos + 2..];
            if let Some(meta) = self
                .all_structs
                .iter()
                .find(|s| s.original_name == struct_name && s.module_name == mod_name)
            {
                if meta.module_name == self.current_module || meta.is_pub {
                    let mangled = mangle_name(&meta.module_name, struct_name, false);
                    return Ok(Type::Struct(mangled.to_string()));
                }
            }
        }

        if let Some(meta) = self.lookup_struct_meta(name) {
            let mangled = mangle_name(&meta.module_name, name, false);
            Ok(Type::Struct(mangled.to_string()))
        } else {
            Err(format!(
                "Struct '{}' not found or is private in module '{}'",
                name, self.current_module
            ))
        }
    }

    fn lookup_struct_meta(&self, name: &str) -> Option<&StructMeta<'a>> {
        if let Some(meta) = self
            .all_structs
            .iter()
            .find(|s| s.original_name == name && s.module_name == self.current_module)
        {
            return Some(meta);
        }
        let empty = Vec::new();
        let imps = self.imports.get(&self.current_module).unwrap_or(&empty);
        for imp in imps {
            if let Some(meta) = self
                .all_structs
                .iter()
                .find(|s| s.original_name == name && s.module_name == *imp && s.is_pub)
            {
                return Some(meta);
            }
        }
        None
    }

    pub fn resolve_function(&self, name: &str) -> Result<&FunctionMeta<'a>, String> {
        if let Some(pos) = name.rfind("::") {
            let mod_name = &name[..pos];
            let func_name = &name[pos + 2..];
            if let Some(meta) = self
                .all_functions
                .iter()
                .find(|f| f.original_name == func_name && f.module_name == mod_name)
            {
                if meta.module_name == self.current_module || meta.is_pub {
                    return Ok(meta);
                }
            }
        }

        if let Some(meta) = self
            .all_functions
            .iter()
            .find(|f| f.original_name == name && f.module_name == self.current_module)
        {
            return Ok(meta);
        }
        let empty = Vec::new();
        let imps = self.imports.get(&self.current_module).unwrap_or(&empty);
        for imp in imps {
            if let Some(meta) = self
                .all_functions
                .iter()
                .find(|f| f.original_name == name && f.module_name == *imp && f.is_pub)
            {
                return Ok(meta);
            }
        }
        Err(format!(
            "Function '{}' not found or is private in module '{}'",
            name, self.current_module
        ))
    }
}

pub trait OptionExt<T> {
    fn ok_ok_or_else<F: FnOnce() -> String>(self, err: F) -> Result<T, String>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_ok_or_else<F: FnOnce() -> String>(self, err: F) -> Result<T, String> {
        match self {
            Some(v) => Ok(v),
            None => Err(err()),
        }
    }
}
