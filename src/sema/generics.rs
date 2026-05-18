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
        crate::hir::Statement::AssertLet { name, is_mut, value } => crate::hir::Statement::AssertLet {
            name,
            is_mut,
            value: substitute_expr(value, mapping),
        },
        crate::hir::Statement::HandleLet { name, is_mut, value, ok_body, err_body, is_ok_escape } => crate::hir::Statement::HandleLet {
            name,
            is_mut,
            value: substitute_expr(value, mapping),
            ok_body: substitute_block(ok_body, mapping),
            err_body: substitute_block(err_body, mapping),
            is_ok_escape,
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
        crate::hir::Statement::InlineFor { var_name, start, end, body } => {
            crate::hir::Statement::InlineFor {
                var_name,
                start: substitute_expr(start, mapping),
                end: substitute_expr(end, mapping),
                body: substitute_block(body, mapping),
            }
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
        crate::hir::Expr::MethodCall { expr: sub, method, args } => crate::hir::Expr::MethodCall {
            expr: Box::new(substitute_expr(*sub, mapping)),
            method,
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
        crate::hir::Expr::IndexAccess { expr, index } => crate::hir::Expr::IndexAccess {
            expr: Box::new(substitute_expr(*expr, mapping)),
            index: Box::new(substitute_expr(*index, mapping)),
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

pub fn unroll_inline_for_block<'a>(
    block: crate::hir::Block<'a>,
    others_len: i64,
) -> Result<crate::hir::Block<'a>, String> {
    let mut new_statements = Vec::new();
    for stmt in block.statements {
        if let crate::hir::Statement::InlineFor { var_name, start, end, body } = stmt {
            let start_val = match eval_const_expr(&start, others_len)? {
                Some(val) => val,
                None => return Err("inline for loop start bound must be a compile-time constant integer".to_string()),
            };
            let end_val = match eval_const_expr(&end, others_len)? {
                Some(val) => val,
                None => return Err("inline for loop end bound must be a compile-time constant integer".to_string()),
            };

            for idx in start_val..end_val {
                let unrolled_body = subst_loop_var_in_block(body.clone(), var_name, idx, others_len)?;
                new_statements.extend(unrolled_body.statements);
            }
        } else {
            new_statements.push(subst_loop_var_in_statement(stmt, others_len)?);
        }
    }
    Ok(crate::hir::Block { statements: new_statements })
}

fn eval_const_expr(expr: &crate::hir::Expr, others_len: i64) -> Result<Option<i64>, String> {
    match expr {
        crate::hir::Expr::Int(val) => Ok(Some(*val)),
        crate::hir::Expr::FieldAccess { expr: sub, field } => {
            if *field == "len" {
                if let crate::hir::Expr::Ident("others") = &**sub {
                    return Ok(Some(others_len));
                }
            }
            Ok(None)
        }
        crate::hir::Expr::Binary { op, lhs, rhs } => {
            let l = eval_const_expr(lhs, others_len)?;
            let r = eval_const_expr(rhs, others_len)?;
            if let (Some(l_val), Some(r_val)) = (l, r) {
                match op {
                    crate::parser::BinOp::Add => Ok(Some(l_val + r_val)),
                    crate::parser::BinOp::Sub => Ok(Some(l_val - r_val)),
                    crate::parser::BinOp::Mul => Ok(Some(l_val * r_val)),
                    crate::parser::BinOp::Div => {
                        if r_val == 0 {
                            Err("Division by zero in compile-time constant evaluation".to_string())
                        } else {
                            Ok(Some(l_val / r_val))
                        }
                    }
                    _ => Ok(None),
                }
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

fn subst_loop_var_in_block<'a>(
    block: crate::hir::Block<'a>,
    var_name: &str,
    var_val: i64,
    others_len: i64,
) -> Result<crate::hir::Block<'a>, String> {
    let mut statements = Vec::new();
    for stmt in block.statements {
        statements.push(subst_loop_var_in_statement_with_var(stmt, var_name, var_val, others_len)?);
    }
    Ok(crate::hir::Block { statements })
}

fn subst_loop_var_in_statement_with_var<'a>(
    stmt: crate::hir::Statement<'a>,
    var_name: &str,
    var_val: i64,
    others_len: i64,
) -> Result<crate::hir::Statement<'a>, String> {
    match stmt {
        crate::hir::Statement::Let { name, is_mut, value } => Ok(crate::hir::Statement::Let {
            name,
            is_mut,
            value: subst_loop_var_in_expr_with_var(value, var_name, var_val, others_len)?,
        }),
        crate::hir::Statement::AssertLet { name, is_mut, value } => Ok(crate::hir::Statement::AssertLet {
            name,
            is_mut,
            value: subst_loop_var_in_expr_with_var(value, var_name, var_val, others_len)?,
        }),
        crate::hir::Statement::HandleLet { name, is_mut, value, ok_body, err_body, is_ok_escape } => Ok(crate::hir::Statement::HandleLet {
            name,
            is_mut,
            value: subst_loop_var_in_expr_with_var(value, var_name, var_val, others_len)?,
            ok_body: subst_loop_var_in_block(ok_body, var_name, var_val, others_len)?,
            err_body: subst_loop_var_in_block(err_body, var_name, var_val, others_len)?,
            is_ok_escape,
        }),
        crate::hir::Statement::Assign { name, is_deref, value } => Ok(crate::hir::Statement::Assign {
            name,
            is_deref,
            value: subst_loop_var_in_expr_with_var(value, var_name, var_val, others_len)?,
        }),
        crate::hir::Statement::AssignField { expr, field, value } => {
            Ok(crate::hir::Statement::AssignField {
                expr: subst_loop_var_in_expr_with_var(expr, var_name, var_val, others_len)?,
                field,
                value: subst_loop_var_in_expr_with_var(value, var_name, var_val, others_len)?,
            })
        }
        crate::hir::Statement::Expr(expr) => {
            Ok(crate::hir::Statement::Expr(subst_loop_var_in_expr_with_var(expr, var_name, var_val, others_len)?))
        }
        crate::hir::Statement::Return(opt_expr) => {
            if let Some(e) = opt_expr {
                Ok(crate::hir::Statement::Return(Some(subst_loop_var_in_expr_with_var(e, var_name, var_val, others_len)?)))
            } else {
                Ok(crate::hir::Statement::Return(None))
            }
        }
        crate::hir::Statement::If { cond, then_block, else_block } => {
            Ok(crate::hir::Statement::If {
                cond: subst_loop_var_in_expr_with_var(cond, var_name, var_val, others_len)?,
                then_block: subst_loop_var_in_block(then_block, var_name, var_val, others_len)?,
                else_block: if let Some(eb) = else_block {
                    Some(subst_loop_var_in_block(eb, var_name, var_val, others_len)?)
                } else {
                    None
                },
            })
        }
        crate::hir::Statement::While { cond, body } => {
            Ok(crate::hir::Statement::While {
                cond: subst_loop_var_in_expr_with_var(cond, var_name, var_val, others_len)?,
                body: subst_loop_var_in_block(body, var_name, var_val, others_len)?,
            })
        }
        crate::hir::Statement::InlineFor { var_name: inner_var, start, end, body } => {
            let start_expr = subst_loop_var_in_expr_with_var(start, var_name, var_val, others_len)?;
            let end_expr = subst_loop_var_in_expr_with_var(end, var_name, var_val, others_len)?;
            let start_v = eval_const_expr(&start_expr, others_len)?
                .ok_or_else(|| "inline for loop start bound must be constant".to_string())?;
            let end_v = eval_const_expr(&end_expr, others_len)?
                .ok_or_else(|| "inline for loop end bound must be constant".to_string())?;
            
            let mut unrolled = Vec::new();
            for idx in start_v..end_v {
                let inner_substituted_body = subst_loop_var_in_block(body.clone(), inner_var, idx, others_len)?;
                let fully_substituted_body = subst_loop_var_in_block(inner_substituted_body, var_name, var_val, others_len)?;
                unrolled.extend(fully_substituted_body.statements);
            }
            Ok(crate::hir::Statement::Comptime(crate::hir::Block { statements: unrolled }))
        }
        other => Ok(other),
    }
}

fn subst_loop_var_in_expr_with_var<'a>(
    expr: crate::hir::Expr<'a>,
    var_name: &str,
    var_val: i64,
    others_len: i64,
) -> Result<crate::hir::Expr<'a>, String> {
    match expr {
        crate::hir::Expr::Ident(name) => {
            if name == var_name {
                Ok(crate::hir::Expr::Int(var_val))
            } else {
                Ok(crate::hir::Expr::Ident(name))
            }
        }
        crate::hir::Expr::FieldAccess { expr: sub, field } => {
            let new_sub = subst_loop_var_in_expr_with_var(*sub, var_name, var_val, others_len)?;
            if field == "len" {
                if let crate::hir::Expr::Ident("others") = &new_sub {
                    return Ok(crate::hir::Expr::Int(others_len));
                }
            }
            Ok(crate::hir::Expr::FieldAccess {
                expr: Box::new(new_sub),
                field,
            })
        }
        crate::hir::Expr::IndexAccess { expr: sub, index } => {
            let new_sub = subst_loop_var_in_expr_with_var(*sub, var_name, var_val, others_len)?;
            let new_index = subst_loop_var_in_expr_with_var(*index, var_name, var_val, others_len)?;
            if let crate::hir::Expr::Ident("others") = &new_sub {
                if let crate::hir::Expr::Int(idx) = new_index {
                    let name = format!("others__{}", idx);
                    return Ok(crate::hir::Expr::Ident(Box::leak(name.into_boxed_str())));
                } else {
                    return Err(format!("others subscript index must be constant, found {:?}", new_index));
                }
            }
            Ok(crate::hir::Expr::IndexAccess {
                expr: Box::new(new_sub),
                index: Box::new(new_index),
            })
        }
        crate::hir::Expr::Binary { op, lhs, rhs } => {
            Ok(crate::hir::Expr::Binary {
                op,
                lhs: Box::new(subst_loop_var_in_expr_with_var(*lhs, var_name, var_val, others_len)?),
                rhs: Box::new(subst_loop_var_in_expr_with_var(*rhs, var_name, var_val, others_len)?),
            })
        }
        crate::hir::Expr::Unary { op, expr } => {
            Ok(crate::hir::Expr::Unary {
                op,
                expr: Box::new(subst_loop_var_in_expr_with_var(*expr, var_name, var_val, others_len)?),
            })
        }
        crate::hir::Expr::Call { name, type_args, args } => {
            let mut new_args = Vec::new();
            for a in args {
                new_args.push(crate::hir::CallArg {
                    name: a.name,
                    value: subst_loop_var_in_expr_with_var(a.value, var_name, var_val, others_len)?,
                });
            }
            Ok(crate::hir::Expr::Call {
                name,
                type_args,
                args: new_args,
            })
        }
        crate::hir::Expr::MethodCall { expr: sub, method, args } => {
            let mut new_args = Vec::new();
            for a in args {
                new_args.push(crate::hir::CallArg {
                    name: a.name,
                    value: subst_loop_var_in_expr_with_var(a.value, var_name, var_val, others_len)?,
                });
            }
            Ok(crate::hir::Expr::MethodCall {
                expr: Box::new(subst_loop_var_in_expr_with_var(*sub, var_name, var_val, others_len)?),
                method,
                args: new_args,
            })
        }
        crate::hir::Expr::As { expr, ty } => {
            Ok(crate::hir::Expr::As {
                expr: Box::new(subst_loop_var_in_expr_with_var(*expr, var_name, var_val, others_len)?),
                ty,
            })
        }
        crate::hir::Expr::Is { expr, ty } => {
            Ok(crate::hir::Expr::Is {
                expr: Box::new(subst_loop_var_in_expr_with_var(*expr, var_name, var_val, others_len)?),
                ty,
            })
        }
        crate::hir::Expr::BuiltinCall { name, args } => {
            let mut new_args = Vec::new();
            for a in args {
                new_args.push(subst_loop_var_in_expr_with_var(a, var_name, var_val, others_len)?);
            }
            Ok(crate::hir::Expr::BuiltinCall { name, args: new_args })
        }
        crate::hir::Expr::Borrow { is_mut, expr } => {
            Ok(crate::hir::Expr::Borrow {
                is_mut,
                expr: Box::new(subst_loop_var_in_expr_with_var(*expr, var_name, var_val, others_len)?),
            })
        }
        crate::hir::Expr::Deref(expr) => {
            Ok(crate::hir::Expr::Deref(Box::new(subst_loop_var_in_expr_with_var(*expr, var_name, var_val, others_len)?)))
        }
        crate::hir::Expr::StructLiteral { name, fields } => {
            let mut new_fields = Vec::new();
            for f in fields {
                new_fields.push(crate::hir::FieldInit {
                    name: f.name,
                    value: subst_loop_var_in_expr_with_var(f.value, var_name, var_val, others_len)?,
                });
            }
            Ok(crate::hir::Expr::StructLiteral { name, fields: new_fields })
        }
        other => Ok(other),
    }
}

fn subst_loop_var_in_statement<'a>(
    stmt: crate::hir::Statement<'a>,
    others_len: i64,
) -> Result<crate::hir::Statement<'a>, String> {
    match stmt {
        crate::hir::Statement::Let { name, is_mut, value } => Ok(crate::hir::Statement::Let {
            name,
            is_mut,
            value: subst_loop_var_in_expr(value, others_len)?,
        }),
        crate::hir::Statement::AssertLet { name, is_mut, value } => Ok(crate::hir::Statement::AssertLet {
            name,
            is_mut,
            value: subst_loop_var_in_expr(value, others_len)?,
        }),
        crate::hir::Statement::HandleLet { name, is_mut, value, ok_body, err_body, is_ok_escape } => Ok(crate::hir::Statement::HandleLet {
            name,
            is_mut,
            value: subst_loop_var_in_expr(value, others_len)?,
            ok_body: unroll_inline_for_block(ok_body, others_len)?,
            err_body: unroll_inline_for_block(err_body, others_len)?,
            is_ok_escape,
        }),
        crate::hir::Statement::Assign { name, is_deref, value } => Ok(crate::hir::Statement::Assign {
            name,
            is_deref,
            value: subst_loop_var_in_expr(value, others_len)?,
        }),
        crate::hir::Statement::AssignField { expr, field, value } => {
            Ok(crate::hir::Statement::AssignField {
                expr: subst_loop_var_in_expr(expr, others_len)?,
                field,
                value: subst_loop_var_in_expr(value, others_len)?,
            })
        }
        crate::hir::Statement::Expr(expr) => {
            Ok(crate::hir::Statement::Expr(subst_loop_var_in_expr(expr, others_len)?))
        }
        crate::hir::Statement::Return(opt_expr) => {
            if let Some(e) = opt_expr {
                Ok(crate::hir::Statement::Return(Some(subst_loop_var_in_expr(e, others_len)?)))
            } else {
                Ok(crate::hir::Statement::Return(None))
            }
        }
        crate::hir::Statement::If { cond, then_block, else_block } => {
            Ok(crate::hir::Statement::If {
                cond: subst_loop_var_in_expr(cond, others_len)?,
                then_block: unroll_inline_for_block(then_block, others_len)?,
                else_block: if let Some(eb) = else_block {
                    Some(unroll_inline_for_block(eb, others_len)?)
                } else {
                    None
                },
            })
        }
        crate::hir::Statement::While { cond, body } => {
            Ok(crate::hir::Statement::While {
                cond: subst_loop_var_in_expr(cond, others_len)?,
                body: unroll_inline_for_block(body, others_len)?,
            })
        }
        crate::hir::Statement::InlineFor { var_name, start, end, body } => {
            let start_expr = subst_loop_var_in_expr(start, others_len)?;
            let end_expr = subst_loop_var_in_expr(end, others_len)?;
            let start_v = eval_const_expr(&start_expr, others_len)?
                .ok_or_else(|| "inline for loop start bound must be constant".to_string())?;
            let end_v = eval_const_expr(&end_expr, others_len)?
                .ok_or_else(|| "inline for loop end bound must be constant".to_string())?;
            
            let mut unrolled = Vec::new();
            for idx in start_v..end_v {
                let inner_substituted_body = subst_loop_var_in_block(body.clone(), var_name, idx, others_len)?;
                unrolled.extend(inner_substituted_body.statements);
            }
            Ok(crate::hir::Statement::Comptime(crate::hir::Block { statements: unrolled }))
        }
        other => Ok(other),
    }
}

fn subst_loop_var_in_expr<'a>(
    expr: crate::hir::Expr<'a>,
    others_len: i64,
) -> Result<crate::hir::Expr<'a>, String> {
    match expr {
        crate::hir::Expr::FieldAccess { expr: sub, field } => {
            let new_sub = subst_loop_var_in_expr(*sub, others_len)?;
            if field == "len" {
                if let crate::hir::Expr::Ident("others") = &new_sub {
                    return Ok(crate::hir::Expr::Int(others_len));
                }
            }
            Ok(crate::hir::Expr::FieldAccess {
                expr: Box::new(new_sub),
                field,
            })
        }
        crate::hir::Expr::IndexAccess { expr: sub, index } => {
            let new_sub = subst_loop_var_in_expr(*sub, others_len)?;
            let new_index = subst_loop_var_in_expr(*index, others_len)?;
            if let crate::hir::Expr::Ident("others") = &new_sub {
                if let crate::hir::Expr::Int(idx) = new_index {
                    let name = format!("others__{}", idx);
                    return Ok(crate::hir::Expr::Ident(Box::leak(name.into_boxed_str())));
                }
            }
            Ok(crate::hir::Expr::IndexAccess {
                expr: Box::new(new_sub),
                index: Box::new(new_index),
            })
        }
        crate::hir::Expr::Binary { op, lhs, rhs } => {
            Ok(crate::hir::Expr::Binary {
                op,
                lhs: Box::new(subst_loop_var_in_expr(*lhs, others_len)?),
                rhs: Box::new(subst_loop_var_in_expr(*rhs, others_len)?),
            })
        }
        crate::hir::Expr::Unary { op, expr } => {
            Ok(crate::hir::Expr::Unary {
                op,
                expr: Box::new(subst_loop_var_in_expr(*expr, others_len)?),
            })
        }
        crate::hir::Expr::Call { name, type_args, args } => {
            let mut new_args = Vec::new();
            for a in args {
                new_args.push(crate::hir::CallArg {
                    name: a.name,
                    value: subst_loop_var_in_expr(a.value, others_len)?,
                });
            }
            Ok(crate::hir::Expr::Call {
                name,
                type_args,
                args: new_args,
            })
        }
        crate::hir::Expr::MethodCall { expr: sub, method, args } => {
            let mut new_args = Vec::new();
            for a in args {
                new_args.push(crate::hir::CallArg {
                    name: a.name,
                    value: subst_loop_var_in_expr(a.value, others_len)?,
                });
            }
            Ok(crate::hir::Expr::MethodCall {
                expr: Box::new(subst_loop_var_in_expr(*sub, others_len)?),
                method,
                args: new_args,
            })
        }
        crate::hir::Expr::As { expr, ty } => {
            Ok(crate::hir::Expr::As {
                expr: Box::new(subst_loop_var_in_expr(*expr, others_len)?),
                ty,
            })
        }
        crate::hir::Expr::Is { expr, ty } => {
            Ok(crate::hir::Expr::Is {
                expr: Box::new(subst_loop_var_in_expr(*expr, others_len)?),
                ty,
            })
        }
        crate::hir::Expr::BuiltinCall { name, args } => {
            let mut new_args = Vec::new();
            for a in args {
                new_args.push(subst_loop_var_in_expr(a, others_len)?);
            }
            Ok(crate::hir::Expr::BuiltinCall { name, args: new_args })
        }
        crate::hir::Expr::Borrow { is_mut, expr } => {
            Ok(crate::hir::Expr::Borrow {
                is_mut,
                expr: Box::new(subst_loop_var_in_expr(*expr, others_len)?),
            })
        }
        crate::hir::Expr::Deref(expr) => {
            Ok(crate::hir::Expr::Deref(Box::new(subst_loop_var_in_expr(*expr, others_len)?)))
        }
        crate::hir::Expr::StructLiteral { name, fields } => {
            let mut new_fields = Vec::new();
            for f in fields {
                new_fields.push(crate::hir::FieldInit {
                    name: f.name,
                    value: subst_loop_var_in_expr(f.value, others_len)?,
                });
            }
            Ok(crate::hir::Expr::StructLiteral { name, fields: new_fields })
        }
        other => Ok(other),
    }
}

