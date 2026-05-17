use std::collections::HashMap;
use super::ast::Type;
use super::context::SemaContext;

pub fn type_to_hir(ty: &Type) -> crate::hir::HirType {
    match ty {
        Type::Void => crate::hir::HirType::Void,
        Type::I64 => crate::hir::HirType::I64,
        Type::F64 => crate::hir::HirType::F64,
        Type::Bool => crate::hir::HirType::Bool,
        Type::Char => crate::hir::HirType::Char,
        Type::Str => crate::hir::HirType::Str,
        Type::TypeVal => crate::hir::HirType::TypeVal,
        Type::Ref { is_mut, ty } => crate::hir::HirType::Ref {
            is_mut: *is_mut,
            ty: Box::new(type_to_hir(ty)),
        },
        Type::Struct(name) => crate::hir::HirType::Struct(name.clone()),
    }
}

pub fn substitute_type(
    ty: &crate::hir::HirType,
    mapping: &HashMap<&str, &crate::hir::HirType>,
) -> crate::hir::HirType {
    match ty {
        crate::hir::HirType::Struct(name) => {
            if let Some(concrete) = mapping.get(name.as_str()) {
                (*concrete).clone()
            } else {
                ty.clone()
            }
        }
        crate::hir::HirType::Ref { is_mut, ty: inner } => crate::hir::HirType::Ref {
            is_mut: *is_mut,
            ty: Box::new(substitute_type(inner, mapping)),
        },
        other => other.clone(),
    }
}

pub fn substitute_statement<'a>(
    stmt: crate::hir::Statement<'a>,
    mapping: &HashMap<&str, &crate::hir::HirType>,
) -> crate::hir::Statement<'a> {
    match stmt {
        crate::hir::Statement::Let { name, is_mut, value } => crate::hir::Statement::Let {
            name,
            is_mut,
            value: substitute_expr(value, mapping),
        },
        crate::hir::Statement::Assign { name, is_deref, value } => crate::hir::Statement::Assign {
            name,
            is_deref,
            value: substitute_expr(value, mapping),
        },
        crate::hir::Statement::AssignField { expr, field, value } => {
            crate::hir::Statement::AssignField {
                expr: substitute_expr(expr, mapping),
                field,
                value: substitute_expr(value, mapping),
            }
        }
        crate::hir::Statement::Expr(expr) => {
            crate::hir::Statement::Expr(substitute_expr(expr, mapping))
        }
        crate::hir::Statement::Return(opt_expr) => {
            crate::hir::Statement::Return(opt_expr.map(|e| substitute_expr(e, mapping)))
        }
        crate::hir::Statement::If {
            cond,
            then_block,
            else_block,
        } => crate::hir::Statement::If {
            cond: substitute_expr(cond, mapping),
            then_block: substitute_block(then_block, mapping),
            else_block: else_block.map(|b| substitute_block(b, mapping)),
        },
        crate::hir::Statement::While { cond, body } => crate::hir::Statement::While {
            cond: substitute_expr(cond, mapping),
            body: substitute_block(body, mapping),
        },
        crate::hir::Statement::Comptime(body) => {
            crate::hir::Statement::Comptime(substitute_block(body, mapping))
        }
        other => other,
    }
}

pub fn substitute_block<'a>(
    block: crate::hir::Block<'a>,
    mapping: &HashMap<&str, &crate::hir::HirType>,
) -> crate::hir::Block<'a> {
    crate::hir::Block {
        statements: block
            .statements
            .into_iter()
            .map(|s| substitute_statement(s, mapping))
            .collect(),
    }
}

pub fn substitute_expr<'a>(
    expr: crate::hir::Expr<'a>,
    mapping: &HashMap<&str, &crate::hir::HirType>,
) -> crate::hir::Expr<'a> {
    match expr {
        crate::hir::Expr::Binary { op, lhs, rhs } => crate::hir::Expr::Binary {
            op,
            lhs: Box::new(substitute_expr(*lhs, mapping)),
            rhs: Box::new(substitute_expr(*rhs, mapping)),
        },
        crate::hir::Expr::Unary { op, expr } => crate::hir::Expr::Unary {
            op,
            expr: Box::new(substitute_expr(*expr, mapping)),
        },
        crate::hir::Expr::Call {
            name,
            type_args,
            args,
        } => crate::hir::Expr::Call {
            name,
            type_args: type_args
                .into_iter()
                .map(|t| substitute_type(&t, mapping))
                .collect(),
            args: args.into_iter().map(|a| crate::hir::CallArg {
                name: a.name,
                value: substitute_expr(a.value, mapping),
            }).collect(),
        },
        crate::hir::Expr::As { expr, ty } => crate::hir::Expr::As {
            expr: Box::new(substitute_expr(*expr, mapping)),
            ty: substitute_type(&ty, mapping),
        },
        crate::hir::Expr::Is { expr, ty } => crate::hir::Expr::Is {
            expr: Box::new(substitute_expr(*expr, mapping)),
            ty: substitute_type(&ty, mapping),
        },
        crate::hir::Expr::BuiltinCall { name, args } => crate::hir::Expr::BuiltinCall {
            name,
            args: args.into_iter().map(|a| substitute_expr(a, mapping)).collect(),
        },
        crate::hir::Expr::Borrow { is_mut, expr } => crate::hir::Expr::Borrow {
            is_mut,
            expr: Box::new(substitute_expr(*expr, mapping)),
        },
        crate::hir::Expr::Deref(expr) => {
            crate::hir::Expr::Deref(Box::new(substitute_expr(*expr, mapping)))
        }
        crate::hir::Expr::StructLiteral { name, fields } => {
            let res_name = if let Some(crate::hir::HirType::Struct(n)) = mapping.get(name) {
                Box::leak(n.clone().into_boxed_str())
            } else {
                name
            };
            crate::hir::Expr::StructLiteral {
                name: res_name,
                fields: fields
                    .into_iter()
                    .map(|f| crate::hir::FieldInit {
                        name: f.name,
                        value: substitute_expr(f.value, mapping),
                    })
                    .collect(),
            }
        }
        crate::hir::Expr::FieldAccess { expr, field } => crate::hir::Expr::FieldAccess {
            expr: Box::new(substitute_expr(*expr, mapping)),
            field,
        },
        other => other,
    }
}

pub fn get_mangled_mono_name(name: &str, type_args: &[Type]) -> String {
    let mut res = name.to_string();
    for t in type_args {
        res.push('_');
        match t {
            Type::I64 => res.push_str("i64"),
            Type::F64 => res.push_str("f64"),
            Type::Bool => res.push_str("bool"),
            Type::Char => res.push_str("char"),
            Type::Str => res.push_str("str"),
            Type::TypeVal => res.push_str("type"),
            Type::Void => res.push_str("void"),
            Type::Ref { is_mut, ty } => {
                res.push_str(if *is_mut { "mutref_" } else { "ref_" });
                res.push_str(
                    &format!("{:?}", ty)
                        .to_lowercase()
                        .replace(":", "_")
                        .replace(" ", ""),
                );
            }
            Type::Struct(s) => res.push_str(&s.to_lowercase().replace(":", "_").replace(" ", "")),
        }
    }
    res
}

pub fn resolve_generic_template<'a, 'b>(
    ctx: &'b SemaContext<'a>,
    name: &str,
) -> Option<&'b crate::hir::Function<'a>> {
    if let Some(pos) = name.rfind("::") {
        let mod_name = &name[..pos];
        let func_name = &name[pos + 2..];
        if let Some(f) = ctx.generic_templates.get(func_name) {
            if f.module_name == mod_name && (f.module_name == ctx.current_module || f.is_pub) {
                return Some(f);
            }
        }
    }

    if let Some(f) = ctx.generic_templates.get(name) {
        if f.module_name == ctx.current_module {
            return Some(f);
        }
    }

    let empty = Vec::new();
    let imps = ctx.imports.get(&ctx.current_module).unwrap_or(&empty);
    for imp in imps {
        if let Some(f) = ctx.generic_templates.get(name) {
            if f.module_name == *imp && f.is_pub {
                return Some(f);
            }
        }
    }
    None
}
