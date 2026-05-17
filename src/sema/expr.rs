use std::collections::HashMap;
use crate::parser::{BinOp, UnOp};
use super::ast::{ExprKind, FieldInit, Function, Param, Type, TypedExpr};
use super::context::{mangle_name, OptionExt, SemaContext, FunctionMeta};
use super::generics::{
    get_mangled_mono_name, resolve_generic_template, substitute_block, substitute_type,
    substitute_expr, type_to_hir,
};
use super::analyze::{get_type_id, get_type_name_slug};
use super::statement::check_block;

pub fn check_expr<'a>(
    ctx: &mut SemaContext<'a>,
    expr: crate::hir::Expr<'a>,
) -> Result<TypedExpr<'a>, String> {
    match expr {
        crate::hir::Expr::Ident(name) => {
            let (ty, _) = ctx
                .lookup_var(name)
                .ok_ok_or_else(|| format!("Variable '{}' not found in scope", name))?;
            Ok(TypedExpr {
                kind: ExprKind::Ident(name),
                ty,
            })
        }
        crate::hir::Expr::Int(val) => Ok(TypedExpr {
            kind: ExprKind::Int(val),
            ty: Type::I64,
        }),
        crate::hir::Expr::Float(val) => Ok(TypedExpr {
            kind: ExprKind::Float(val),
            ty: Type::F64,
        }),
        crate::hir::Expr::Bool(val) => Ok(TypedExpr {
            kind: ExprKind::Bool(val),
            ty: Type::Bool,
        }),
        crate::hir::Expr::Str(val) => {
            let ty = Type::Ref {
                is_mut: false,
                ty: Box::new(Type::Str),
            };
            Ok(TypedExpr {
                kind: ExprKind::Str(val),
                ty,
            })
        }
        crate::hir::Expr::Unary { op, expr } => {
            let typed_expr = check_expr(ctx, *expr)?;
            let res_ty = match op {
                UnOp::Not => {
                    if typed_expr.ty != Type::Bool {
                        return Err(format!(
                            "Unary '!' operator cannot be applied to {:?}",
                            typed_expr.ty
                        ));
                    }
                    Type::Bool
                }
                UnOp::Neg => {
                    if typed_expr.ty != Type::I64 && typed_expr.ty != Type::F64 {
                        return Err(format!(
                            "Unary '-' operator cannot be applied to {:?}",
                            typed_expr.ty
                        ));
                    }
                    typed_expr.ty.clone()
                }
            };
            Ok(TypedExpr {
                kind: ExprKind::Unary {
                    op,
                    expr: Box::new(typed_expr),
                },
                ty: res_ty,
            })
        }
        crate::hir::Expr::Binary { op, lhs, rhs } => {
            let typed_lhs = check_expr(ctx, *lhs)?;
            let typed_rhs = check_expr(ctx, *rhs)?;

            let res_ty = match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                    if typed_lhs.ty == Type::I64 && typed_rhs.ty == Type::I64 {
                        Type::I64
                    } else if typed_lhs.ty == Type::F64 && typed_rhs.ty == Type::F64 {
                        Type::F64
                    } else {
                        return Err(format!("Arithmetic operator {:?} requires matching I64 or F64 operands, found {:?} and {:?}", op, typed_lhs.ty, typed_rhs.ty));
                    }
                }
                BinOp::And | BinOp::Or => {
                    if typed_lhs.ty != Type::Bool || typed_rhs.ty != Type::Bool {
                        return Err(format!(
                            "Logical operator {:?} requires Bool operands, found {:?} and {:?}",
                            op, typed_lhs.ty, typed_rhs.ty
                        ));
                    }
                    Type::Bool
                }
                BinOp::Eq | BinOp::Ne => {
                    if typed_lhs.ty != typed_rhs.ty {
                        return Err(format!("Comparison operator {:?} requires matching operand types, found {:?} and {:?}", op, typed_lhs.ty, typed_rhs.ty));
                    }
                    Type::Bool
                }
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    if (typed_lhs.ty == Type::I64 && typed_rhs.ty == Type::I64)
                        || (typed_lhs.ty == Type::F64 && typed_rhs.ty == Type::F64)
                    {
                        Type::Bool
                    } else {
                        return Err(format!("Relational operator {:?} requires matching I64 or F64 operands, found {:?} and {:?}", op, typed_lhs.ty, typed_rhs.ty));
                    }
                }
            };

            Ok(TypedExpr {
                kind: ExprKind::Binary {
                    op,
                    lhs: Box::new(typed_lhs),
                    rhs: Box::new(typed_rhs),
                },
                ty: res_ty,
            })
        }
        crate::hir::Expr::Call {
            name,
            type_args,
            args,
        } => {
            let opt_template = resolve_generic_template(ctx, name).cloned();
            let has_others = if let Some(ref template) = opt_template {
                template.params.iter().any(|p| p.name == "others")
            } else {
                false
            };

            let args = if has_others {
                let mut positional = Vec::new();
                for arg in args {
                    if arg.name.is_some() {
                        return Err("Named arguments are not supported for variadic functions".to_string());
                    }
                    positional.push(arg.value);
                }
                positional
            } else {
                let (param_names, param_defaults) = if let Some(template) = &opt_template {
                    let names = template.params.iter().map(|p| p.name.to_string()).collect::<Vec<_>>();
                    let defaults = template.params.iter().map(|p| p.default.clone()).collect::<Vec<_>>();
                    (names, defaults)
                } else {
                    let meta = ctx.resolve_function(name)?;
                    (meta.param_names.clone(), meta.param_defaults.clone())
                };

                let n = param_names.len();
                let mut first_default_idx = n;
                for (i, default_opt) in param_defaults.iter().enumerate() {
                    if default_opt.is_some() {
                        first_default_idx = i;
                        break;
                    }
                }

                let mut positional_args = Vec::new();
                let mut named_args = HashMap::new();
                let mut seen_named = false;
                for arg in args {
                    if let Some(arg_name) = arg.name {
                        seen_named = true;
                        if named_args.contains_key(arg_name) {
                            return Err(format!("Duplicate argument for parameter '{}'", arg_name));
                        }
                        named_args.insert(arg_name, arg.value);
                    } else {
                        if seen_named {
                            return Err("Positional arguments must appear before named arguments".to_string());
                        }
                        positional_args.push(arg.value);
                    }
                }

                if positional_args.len() > first_default_idx {
                    return Err(format!(
                        "Function '{}' expects at most {} positional arguments, but {} were provided",
                        name, first_default_idx, positional_args.len()
                    ));
                }

                let mut reordered_args = Vec::new();
                for i in 0..n {
                    if i < positional_args.len() {
                        if named_args.contains_key(param_names[i].as_str()) {
                            return Err(format!("Parameter '{}' is provided both positionally and as a named argument", param_names[i]));
                        }
                        reordered_args.push(positional_args[i].clone());
                    } else {
                        if let Some(val) = named_args.remove(param_names[i].as_str()) {
                            reordered_args.push(val);
                        } else if let Some(ref default_expr) = param_defaults[i] {
                            reordered_args.push(default_expr.clone());
                        } else {
                            return Err(format!("Missing required argument for parameter '{}'", param_names[i]));
                        }
                    }
                }

                if !named_args.is_empty() {
                    let unknown_param = named_args.keys().next().unwrap();
                    return Err(format!("Function '{}' has no parameter named '{}'", name, unknown_param));
                }

                reordered_args
            };

            let (mangled_name, param_tys, ret_ty) = if let Some(template) = opt_template {
                let mut resolved_type_args = Vec::new();
                for t in &type_args {
                    let resolved = ctx.resolve_type(Type::from(t.clone()))?;
                    resolved_type_args.push(resolved);
                }

                if resolved_type_args.len() != template.generics.len() {
                    return Err(format!(
                        "Generic function '{}' expects {} type arguments, found {}",
                        name,
                        template.generics.len(),
                        resolved_type_args.len()
                    ));
                }

                let others_count = if has_others {
                    let normal_params_count = template.params.len() - 1;
                    if args.len() < normal_params_count {
                        return Err(format!(
                            "Function '{}' expects at least {} arguments, found {}",
                            name, normal_params_count, args.len()
                        ));
                    }
                    args.len() - normal_params_count
                } else {
                    0
                };

                let mut mangled_mono_name = get_mangled_mono_name(template.name, &resolved_type_args);
                if has_others {
                    mangled_mono_name.push_str(&format!("_others_{}", others_count));
                }

                if !ctx.functions.contains_key(mangled_mono_name.as_str()) {
                    let mut mapping = HashMap::new();
                    let hir_type_args: Vec<crate::hir::HirType> =
                        resolved_type_args.iter().map(type_to_hir).collect();
                    for (i, gen_param) in template.generics.iter().enumerate() {
                        mapping.insert(*gen_param, &hir_type_args[i]);
                    }

                    // Compile-time Type Gate Check
                    ctx.push_scope();
                    for (i, gen_param) in template.generics.iter().enumerate() {
                        ctx.declare_var(gen_param, resolved_type_args[i].clone(), false);
                    }
                    for (i, gen_param) in template.generics.iter().enumerate() {
                        if let Some(ref contract) = template.generic_contracts[i] {
                            let checked = check_expr(ctx, contract.clone())?;
                            match checked.kind {
                                ExprKind::Bool(true) => {}
                                ExprKind::Bool(false) => {
                                    return Err(format!(
                                        "Generic type constraint violated for parameter '{}': contract evaluated to false",
                                        gen_param
                                    ));
                                }
                                _ => return Err(format!("Generic constraint must evaluate to a boolean constant")),
                            }
                        }
                    }
                    ctx.pop_scope();

                    let sub_params = template
                        .params
                        .iter()
                        .map(|p| crate::hir::Param {
                            name: p.name,
                            ty: substitute_type(&p.ty, &mapping),
                            contract: p.contract.clone().map(|c| substitute_expr(c, &mapping)),
                            default: p.default.clone().map(|d| substitute_expr(d, &mapping)),
                        })
                        .collect::<Vec<_>>();

                    let sub_ret = substitute_type(&template.ret_type, &mapping);
                    let sub_body_substituted =
                        substitute_block(template.body.clone(), &mapping);

                    let mut final_params = Vec::new();
                    for p in sub_params {
                        if p.name == "others" {
                            for idx in 0..others_count {
                                let name = format!("others__{}", idx);
                                final_params.push(crate::hir::Param {
                                    name: Box::leak(name.into_boxed_str()),
                                    ty: p.ty.clone(),
                                    contract: None,
                                    default: None,
                                });
                            }
                        } else {
                            final_params.push(p);
                        }
                    }

                    let sub_body = crate::sema::generics::unroll_inline_for_block(
                        sub_body_substituted,
                        others_count as i64,
                    )?;

                    let sema_param_tys: Vec<Type> = final_params
                        .iter()
                        .map(|p| Type::from(p.ty.clone()))
                        .collect();
                    let sema_ret = Type::from(sub_ret.clone());

                    ctx.functions.insert(
                        Box::leak(mangled_mono_name.clone().into_boxed_str()),
                        (sema_param_tys.clone(), sema_ret.clone()),
                    );

                    let mono_names = final_params.iter().map(|p| p.name.to_string()).collect();
                    let mono_defaults = final_params.iter().map(|p| p.default.clone()).collect();
                    ctx.all_functions.push(FunctionMeta {
                        original_name: Box::leak(mangled_mono_name.clone().into_boxed_str()),
                        module_name: template.module_name.clone(),
                        is_pub: template.is_pub,
                        is_extern: false,
                        param_types: sema_param_tys.clone(),
                        ret_type: sema_ret.clone(),
                        param_names: mono_names,
                        param_defaults: mono_defaults,
                    });

                    let old_ret_type = ctx.current_ret_type.clone();
                    let old_module = ctx.current_module.clone();

                    ctx.current_ret_type = Some(sema_ret.clone());
                    ctx.current_module = template.module_name.clone();
                    ctx.push_scope();

                    let mut params = Vec::new();
                    for (p, ty) in final_params.iter().zip(&sema_param_tys) {
                        ctx.declare_var(p.name, ty.clone(), true);
                        let typed_contract = if let Some(ref c) = p.contract {
                            Some(check_expr(ctx, c.clone())?)
                        } else {
                            None
                        };
                        params.push(Param {
                            name: p.name,
                            ty: ty.clone(),
                            contract: typed_contract,
                        });
                    }

                    let body = check_block(ctx, sub_body)?;
                    ctx.pop_scope();
                    ctx.current_ret_type = old_ret_type;
                    ctx.current_module = old_module;

                    ctx.monomorphized_functions.push(Function {
                        name: Box::leak(mangled_mono_name.clone().into_boxed_str()),
                        params,
                        ret_type: sema_ret,
                        body,
                    });
                }

                let (param_tys, ret_ty) = ctx
                    .functions
                    .get(mangled_mono_name.as_str())
                    .unwrap()
                    .clone();
                (mangled_mono_name, param_tys, ret_ty)
            } else {
                let meta = ctx.resolve_function(name)?;
                let mangled = mangle_name(&meta.module_name, meta.original_name, meta.is_extern);
                let (param_tys, ret_ty) = ctx.functions.get(mangled).unwrap().clone();
                (mangled.to_string(), param_tys, ret_ty)
            };

            if args.len() != param_tys.len() {
                return Err(format!(
                    "Function '{}' expects {} arguments, found {}",
                    name,
                    param_tys.len(),
                    args.len()
                ));
            }

            let mut typed_args = Vec::new();
            for (arg, expected_ty) in args.into_iter().zip(param_tys) {
                let typed_arg = check_expr(ctx, arg)?;
                if typed_arg.ty != expected_ty {
                    return Err(format!(
                        "Argument type mismatch for function '{}': expected {:?}, found {:?}",
                        name, expected_ty, typed_arg.ty
                    ));
                }
                typed_args.push(typed_arg);
            }

            Ok(TypedExpr {
                kind: ExprKind::Call {
                    name: Box::leak(mangled_name.into_boxed_str()),
                    args: typed_args,
                },
                ty: ret_ty,
            })
        }
        crate::hir::Expr::As { expr, ty } => {
            let typed_expr = check_expr(ctx, *expr)?;
            let raw_dest_ty = Type::from(ty);
            let dest_ty = ctx.resolve_type(raw_dest_ty)?;

            // Check casting validity
            let valid = match (&typed_expr.ty, &dest_ty) {
                (Type::I64, Type::F64) | (Type::F64, Type::I64) => true,
                (t1, t2) if t1 == t2 => true,
                _ => false,
            };

            if !valid {
                return Err(format!(
                    "Cannot cast expression from {:?} to {:?}",
                    typed_expr.ty, dest_ty
                ));
            }

            Ok(TypedExpr {
                kind: ExprKind::As {
                    expr: Box::new(typed_expr),
                    ty: dest_ty.clone(),
                },
                ty: dest_ty,
            })
        }
        crate::hir::Expr::Borrow { is_mut, expr } => {
            let typed_expr = check_expr(ctx, *expr)?;
            if is_mut {
                if !is_writable(ctx, &typed_expr) {
                    return Err(format!("Cannot borrow immutable expression as mutable"));
                }
            }

            let ref_ty = Type::Ref {
                is_mut,
                ty: Box::new(typed_expr.ty.clone()),
            };

            Ok(TypedExpr {
                kind: ExprKind::Borrow {
                    is_mut,
                    expr: Box::new(typed_expr),
                },
                ty: ref_ty,
            })
        }
        crate::hir::Expr::Deref(expr) => {
            let typed_expr = check_expr(ctx, *expr)?;
            match &typed_expr.ty {
                Type::Ref { ty: inner_ty, .. } => {
                    let inner = (**inner_ty).clone();
                    Ok(TypedExpr {
                        kind: ExprKind::Deref(Box::new(typed_expr)),
                        ty: inner,
                    })
                }
                other => Err(format!("Cannot dereference non-reference type {:?}", other)),
            }
        }
        crate::hir::Expr::StructLiteral { name, fields } => {
            let resolved_ty = ctx.resolve_struct_type(name)?;
            let Type::Struct(mangled_name) = &resolved_ty else {
                unreachable!()
            };
            let struct_fields = ctx.structs.get(mangled_name).unwrap().clone();

            if fields.len() != struct_fields.len() {
                return Err(format!(
                    "Struct '{}' expects {} fields, found {}",
                    name,
                    struct_fields.len(),
                    fields.len()
                ));
            }

            let mut checked_fields = Vec::new();
            for (expected_name, expected_ty) in &struct_fields {
                let init = fields
                    .iter()
                    .find(|f| f.name == expected_name)
                    .ok_ok_or_else(|| {
                        format!("Missing field '{}' in initializer for '{}'", expected_name, name)
                    })?;
                let checked_val = check_expr(ctx, init.value.clone())?;
                if checked_val.ty != *expected_ty {
                    return Err(format!(
                        "Type mismatch for field '{}' of struct '{}': expected {:?}, found {:?}",
                        expected_name, name, expected_ty, checked_val.ty
                    ));
                }
                checked_fields.push(FieldInit {
                    name: init.name,
                    value: checked_val,
                });
            }

            let mangled_name_ref = Box::leak(mangled_name.clone().into_boxed_str());
            Ok(TypedExpr {
                kind: ExprKind::StructLiteral {
                    name: mangled_name_ref,
                    fields: checked_fields,
                },
                ty: resolved_ty,
            })
        }
        crate::hir::Expr::FieldAccess { expr, field } => {
            let checked_expr = check_expr(ctx, *expr)?;
            let mut current_ty = &checked_expr.ty;
            while let Type::Ref { ty, .. } = current_ty {
                current_ty = ty;
            }
            match current_ty {
                Type::Struct(struct_name) => {
                    let struct_fields = ctx
                        .structs
                        .get(struct_name)
                        .ok_ok_or_else(|| format!("Struct '{}' not defined", struct_name))?;
                    let (_, field_ty) = struct_fields
                        .iter()
                        .find(|(n, _)| n == field)
                        .ok_ok_or_else(|| {
                            format!("No field '{}' on struct '{}'", field, struct_name)
                        })?;

                    Ok(TypedExpr {
                        kind: ExprKind::FieldAccess {
                            expr: Box::new(checked_expr),
                            field,
                        },
                        ty: field_ty.clone(),
                    })
                }
                other => Err(format!(
                    "Cannot access field '{}' on non-struct type {:?}",
                    field, other
                )),
            }
        }
        crate::hir::Expr::Is { expr, ty } => {
            let typed_expr = check_expr(ctx, *expr)?;
            let raw_dest_ty = Type::from(ty);
            let dest_ty = ctx.resolve_type(raw_dest_ty)?;
            let is_match = if let Type::Struct(ref trait_name) = dest_ty {
                if ctx.lookup_trait_meta(trait_name).is_some() {
                    let mut base_ty = &typed_expr.ty;
                    while let Type::Ref { ty, .. } = base_ty {
                        base_ty = ty;
                    }
                    ctx.all_impls.iter().any(|imp| {
                        let mut imp_base_ty = &imp.for_type;
                        while let Type::Ref { ty, .. } = imp_base_ty {
                            imp_base_ty = ty;
                        }
                        imp.trait_name == *trait_name && base_ty == imp_base_ty
                    })
                } else {
                    typed_expr.ty == dest_ty
                }
            } else {
                typed_expr.ty == dest_ty
            };
            Ok(TypedExpr {
                kind: ExprKind::Bool(is_match),
                ty: Type::Bool,
            })
        }
        crate::hir::Expr::MethodCall { expr, method, args } => {
            let typed_expr = check_expr(ctx, *expr)?;
            let mut resolved_impl_fn = None;
            for imp in &ctx.all_impls {
                let mut base_ty = &typed_expr.ty;
                while let Type::Ref { ty, .. } = base_ty {
                    base_ty = ty;
                }
                let mut imp_base_ty = &imp.for_type;
                while let Type::Ref { ty, .. } = imp_base_ty {
                    imp_base_ty = ty;
                }
                if base_ty == imp_base_ty {
                    if let Some(meta) = imp.methods.iter().find(|m| m.original_name == method) {
                        resolved_impl_fn = Some((imp.trait_name.clone(), meta));
                        break;
                    }
                }
            }

            let (mangled_fn_name, param_tys, ret_ty) = if let Some((trait_name, _meta)) = resolved_impl_fn {
                let mut base_ty = &typed_expr.ty;
                while let Type::Ref { ty, .. } = base_ty {
                    base_ty = ty;
                }
                let mangled = format!("{}__{}__{}", trait_name, method, get_type_name_slug(base_ty));
                let mangled_ref = Box::leak(mangled.into_boxed_str());
                let (param_tys, ret_ty) = ctx.functions.get(mangled_ref).ok_ok_or_else(|| {
                    format!("Trait method implementation function '{}' not found in context", mangled_ref)
                })?.clone();
                (mangled_ref, param_tys, ret_ty)
            } else {
                return Err(format!("Method '{}' not found for type {:?}", method, typed_expr.ty));
            };

            let mut all_args = Vec::new();
            all_args.push(typed_expr);
            for arg in args {
                all_args.push(check_expr(ctx, arg.value)?);
            }

            if all_args.len() != param_tys.len() {
                return Err(format!(
                    "Method '{}' expects {} arguments, found {}",
                    method, param_tys.len(), all_args.len()
                ));
            }

            let mut typed_args = Vec::new();
            for (arg, expected_ty) in all_args.into_iter().zip(param_tys) {
                let mut arg = arg;
                if let Type::Ref { is_mut, ty: _ref_inner } = &expected_ty {
                    if !matches!(arg.ty, Type::Ref { .. }) {
                        if *is_mut && !is_writable(ctx, &arg) {
                            return Err(format!("Cannot borrow immutable expression as mutable self receiver"));
                        }
                        arg = TypedExpr {
                            kind: ExprKind::Borrow {
                                is_mut: *is_mut,
                                expr: Box::new(arg.clone()),
                            },
                            ty: expected_ty.clone(),
                        };
                    }
                }
                if arg.ty != expected_ty {
                    return Err(format!(
                        "Argument type mismatch for method '{}': expected {:?}, found {:?}",
                        method, expected_ty, arg.ty
                    ));
                }
                typed_args.push(arg);
            }

            Ok(TypedExpr {
                kind: ExprKind::Call {
                    name: mangled_fn_name,
                    args: typed_args,
                },
                ty: ret_ty,
            })
        }
        crate::hir::Expr::BuiltinCall { name, args } => match name {
            "typeof" => {
                if args.len() != 1 {
                    return Err(format!(
                        "@typeof expects exactly 1 argument, found {}",
                        args.len()
                    ));
                }
                let typed_arg = check_expr(ctx, args[0].clone())?;
                let ty_id = get_type_id(&typed_arg.ty);
                Ok(TypedExpr {
                    kind: ExprKind::Int(ty_id),
                    ty: Type::TypeVal,
                })
            }
            other => Err(format!("Unknown builtin function @{}", other)),
        },
        crate::hir::Expr::IndexAccess { .. } => {
            Err("subscripting is only supported on variadic others".to_string())
        }
    }
}

pub fn is_writable<'a>(ctx: &SemaContext<'a>, expr: &TypedExpr<'a>) -> bool {
    match &expr.kind {
        ExprKind::Ident(name) => {
            if let Some((_, is_mut)) = ctx.lookup_var(name) {
                is_mut
            } else {
                false
            }
        }
        ExprKind::Deref(sub_expr) => match &sub_expr.ty {
            Type::Ref { is_mut, .. } => *is_mut,
            _ => false,
        },
        ExprKind::FieldAccess {
            expr: base_expr, ..
        } => match &base_expr.ty {
            Type::Ref { is_mut, .. } => *is_mut,
            _ => is_writable(ctx, base_expr),
        },
        _ => false,
    }
}
