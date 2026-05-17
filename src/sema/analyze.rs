use super::ast::{ExternFunction, Function, Param, Program, StructDef, Type};
use super::context::{mangle_name, SemaContext, StructMeta, FunctionMeta};
use super::statement::check_block;

pub fn get_type_id(ty: &Type) -> i64 {
    match ty {
        Type::Void => 0,
        Type::I64 => 1,
        Type::F64 => 2,
        Type::Bool => 3,
        Type::Char => 4,
        Type::Str => 5,
        Type::TypeVal => 6,
        Type::Ref { is_mut, ty } => {
            let inner_id = get_type_id(ty);
            let mut_flag = if *is_mut { 1 } else { 0 };
            inner_id * 10 + mut_flag + 10
        }
        Type::Struct(name) => {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            name.hash(&mut hasher);
            let hash = (hasher.finish() & 0x7FFFFFFFFFFFFFFF) as i64;
            if hash < 1000000 {
                hash + 1000000
            } else {
                hash
            }
        }
    }
}

pub fn analyze<'a>(program: crate::hir::Program<'a>) -> Result<Program<'a>, String> {
    let mut ctx = SemaContext::new();

    // Register metadata
    ctx.imports = program.imports;

    // First pass: populate raw structs
    for s in &program.structs {
        let mut fields = Vec::new();
        for f in &s.fields {
            let f_ty = Type::from(f.ty.clone());
            fields.push((f.name.to_string(), f_ty));
        }
        ctx.all_structs.push(StructMeta {
            original_name: s.name,
            module_name: s.module_name.clone(),
            is_pub: s.is_pub,
            fields,
        });
    }

    // First pass: populate raw functions
    let mut normal_functions = Vec::new();
    for f in program.functions {
        if !f.generics.is_empty() {
            ctx.generic_templates.insert(f.name.to_string(), f);
        } else {
            let param_tys: Vec<Type> = f
                .params
                .iter()
                .map(|p| Type::from(p.ty.clone()))
                .collect();
            let ret_ty = Type::from(f.ret_type.clone());
            ctx.all_functions.push(FunctionMeta {
                original_name: f.name,
                module_name: f.module_name.clone(),
                is_pub: f.is_pub,
                is_extern: false,
                param_types: param_tys,
                ret_type: ret_ty,
            });
            normal_functions.push(f);
        }
    }

    for f in &program.extern_functions {
        let param_tys: Vec<Type> = f
            .params
            .iter()
            .map(|p| Type::from(p.ty.clone()))
            .collect();
        let ret_ty = Type::from(f.ret_type.clone());
        ctx.all_functions.push(FunctionMeta {
            original_name: f.name,
            module_name: f.module_name.clone(),
            is_pub: f.is_pub,
            is_extern: true,
            param_types: param_tys,
            ret_type: ret_ty,
        });
    }

    // Resolve all struct field types
    let mut resolved_structs = Vec::new();
    for s in &ctx.all_structs {
        ctx.current_module = s.module_name.clone();
        let mut resolved_fields = Vec::new();
        for (f_name, f_ty) in &s.fields {
            let res_ty = ctx.resolve_type(f_ty.clone())?;
            resolved_fields.push((f_name.clone(), res_ty));
        }
        resolved_structs.push((
            s.module_name.clone(),
            s.original_name.to_string(),
            resolved_fields,
        ));
    }

    // Update all_structs with resolved field types, and populate ctx.structs by their mangled names!
    for (mod_name, orig_name, fields) in resolved_structs {
        let mangled = mangle_name(&mod_name, &orig_name, false);
        ctx.structs.insert(mangled.to_string(), fields.clone());
        if let Some(meta) = ctx
            .all_structs
            .iter_mut()
            .find(|s| s.original_name == orig_name && s.module_name == mod_name)
        {
            meta.fields = fields;
        }
    }

    // Resolve all function signature types
    let mut resolved_functions = Vec::new();
    for f in &ctx.all_functions {
        ctx.current_module = f.module_name.clone();
        let mut resolved_params = Vec::new();
        for p_ty in &f.param_types {
            resolved_params.push(ctx.resolve_type(p_ty.clone())?);
        }
        let resolved_ret = ctx.resolve_type(f.ret_type.clone())?;
        resolved_functions.push((
            f.module_name.clone(),
            f.original_name.to_string(),
            resolved_params,
            resolved_ret,
        ));
    }

    // Update all_functions with resolved signature types, and populate ctx.functions by their mangled names!
    for (mod_name, orig_name, params, ret) in resolved_functions {
        let is_ext = ctx
            .all_functions
            .iter()
            .find(|f| f.original_name == orig_name && f.module_name == mod_name)
            .map(|f| f.is_extern)
            .unwrap_or(false);
        let mangled = mangle_name(&mod_name, &orig_name, is_ext);
        ctx.functions.insert(mangled, (params.clone(), ret.clone()));
        if let Some(meta) = ctx
            .all_functions
            .iter_mut()
            .find(|f| f.original_name == orig_name && f.module_name == mod_name)
        {
            meta.param_types = params;
            meta.ret_type = ret;
        }
    }

    let mut structs = Vec::new();
    for s in &program.structs {
        ctx.current_module = s.module_name.clone();
        let mangled = mangle_name(&s.module_name, s.name, false);
        let fields = ctx.structs.get(mangled).unwrap().clone();
        let sema_fields = fields
            .into_iter()
            .map(|(n, ty)| {
                let orig_f = s.fields.iter().find(|of| of.name == n).unwrap();
                Param {
                    name: orig_f.name,
                    ty,
                }
            })
            .collect();
        structs.push(StructDef {
            name: mangled,
            fields: sema_fields,
        });
    }

    let mut functions = Vec::new();
    for f in normal_functions {
        ctx.current_module = f.module_name.clone();
        ctx.push_scope();

        let mangled = mangle_name(&f.module_name, f.name, false);
        let (param_tys, ret_ty) = ctx.functions.get(mangled).unwrap().clone();
        ctx.current_ret_type = Some(ret_ty.clone());

        let mut params = Vec::new();
        for (p, ty) in f.params.iter().zip(param_tys) {
            ctx.declare_var(p.name, ty.clone(), true);
            params.push(Param { name: p.name, ty });
        }

        let body = check_block(&mut ctx, f.body)?;
        ctx.pop_scope();
        ctx.current_ret_type = None;

        functions.push(Function {
            name: mangled,
            params,
            ret_type: ret_ty,
            body,
        });
    }

    let monomorphized = std::mem::take(&mut ctx.monomorphized_functions);
    functions.extend(monomorphized);

    let mut extern_functions = Vec::new();
    for f in &program.extern_functions {
        ctx.current_module = f.module_name.clone();
        let params = f
            .params
            .iter()
            .map(|p| {
                let ty = Type::from(p.ty.clone());
                let res_ty = ctx.resolve_type(ty).unwrap();
                Param {
                    name: p.name,
                    ty: res_ty,
                }
            })
            .collect();
        let ret_type = ctx.resolve_type(Type::from(f.ret_type.clone())).unwrap();
        let mangled = mangle_name(&f.module_name, f.name, true);
        extern_functions.push(ExternFunction {
            name: mangled,
            params,
            ret_type,
        });
    }

    Ok(Program {
        structs,
        functions,
        extern_functions,
    })
}
