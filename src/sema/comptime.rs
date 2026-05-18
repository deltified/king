use std::collections::HashMap;
use super::ast::Type;
use super::context::{SemaContext, mangle_name, OptionExt};
use super::analyze::get_type_id;

#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeVal {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Type(Type),
    Struct(String, HashMap<String, ComptimeVal>),
    Void,
}

pub fn eval_comptime_expr(
    expr: &crate::hir::Expr,
    env: &mut HashMap<String, ComptimeVal>,
    ctx: &SemaContext,
) -> Result<ComptimeVal, String> {
    match expr {
        crate::hir::Expr::Ident(name) => {
            if let Some(val) = env.get(*name) {
                Ok(val.clone())
            } else if *name == "i64"
                || *name == "f64"
                || *name == "bool"
                || *name == "type"
                || *name == "str"
            {
                let res_ty = ctx.resolve_struct_type(name)?;
                Ok(ComptimeVal::Type(res_ty))
            } else {
                Err(format!(
                    "Compile-time variable '{}' not found in comptime scope",
                    name
                ))
            }
        }
        crate::hir::Expr::Int(val) => Ok(ComptimeVal::Int(*val)),
        crate::hir::Expr::Float(val) => Ok(ComptimeVal::Float(*val)),
        crate::hir::Expr::Bool(val) => Ok(ComptimeVal::Bool(*val)),
        crate::hir::Expr::Str(val) => Ok(ComptimeVal::Str(val.clone())),
        crate::hir::Expr::Binary { op, lhs, rhs } => {
            let left = eval_comptime_expr(lhs, env, ctx)?;
            let right = eval_comptime_expr(rhs, env, ctx)?;
            use crate::parser::BinOp;
            match (op, left, right) {
                (BinOp::Add, ComptimeVal::Int(l), ComptimeVal::Int(r)) => Ok(ComptimeVal::Int(l + r)),
                (BinOp::Sub, ComptimeVal::Int(l), ComptimeVal::Int(r)) => Ok(ComptimeVal::Int(l - r)),
                (BinOp::Mul, ComptimeVal::Int(l), ComptimeVal::Int(r)) => Ok(ComptimeVal::Int(l * r)),
                (BinOp::Div, ComptimeVal::Int(l), ComptimeVal::Int(r)) => {
                    if r == 0 {
                        Err("Division by zero at compile time".to_string())
                    } else {
                        Ok(ComptimeVal::Int(l / r))
                    }
                }
                (BinOp::Eq, ComptimeVal::Int(l), ComptimeVal::Int(r)) => Ok(ComptimeVal::Bool(l == r)),
                (BinOp::Ne, ComptimeVal::Int(l), ComptimeVal::Int(r)) => Ok(ComptimeVal::Bool(l != r)),
                (BinOp::Lt, ComptimeVal::Int(l), ComptimeVal::Int(r)) => Ok(ComptimeVal::Bool(l < r)),
                (BinOp::Le, ComptimeVal::Int(l), ComptimeVal::Int(r)) => Ok(ComptimeVal::Bool(l <= r)),
                (BinOp::Gt, ComptimeVal::Int(l), ComptimeVal::Int(r)) => Ok(ComptimeVal::Bool(l > r)),
                (BinOp::Ge, ComptimeVal::Int(l), ComptimeVal::Int(r)) => Ok(ComptimeVal::Bool(l >= r)),
                (BinOp::Eq, ComptimeVal::Type(l), ComptimeVal::Type(r)) => Ok(ComptimeVal::Bool(l == r)),
                (BinOp::Ne, ComptimeVal::Type(l), ComptimeVal::Type(r)) => Ok(ComptimeVal::Bool(l != r)),
                (BinOp::Eq, ComptimeVal::Bool(l), ComptimeVal::Bool(r)) => Ok(ComptimeVal::Bool(l == r)),
                (BinOp::Ne, ComptimeVal::Bool(l), ComptimeVal::Bool(r)) => Ok(ComptimeVal::Bool(l != r)),
                _ => Err("Unsupported binary operation at compile time".to_string()),
            }
        }
        crate::hir::Expr::Is { expr: sub_expr, ty } => {
            let sub_val = eval_comptime_expr(sub_expr, env, ctx)?;
            let ty_val = match sub_val {
                ComptimeVal::Int(_) => Type::I64,
                ComptimeVal::Float(_) => Type::F64,
                ComptimeVal::Bool(_) => Type::Bool,
                ComptimeVal::Str(_) => Type::Str,
                ComptimeVal::Type(_) => Type::TypeVal,
                ComptimeVal::Struct(name, _) => Type::Struct(name),
                _ => Type::Void,
            };
            let dest_ty = ctx.resolve_type(Type::from(ty.clone()))?;
            Ok(ComptimeVal::Bool(ty_val == dest_ty))
        }
        crate::hir::Expr::BuiltinCall { name, args } => match *name {
            "typeof" => {
                if args.len() != 1 {
                    return Err(format!("@typeof expects 1 argument, found {}", args.len()));
                }
                let arg_val = eval_comptime_expr(&args[0], env, ctx)?;
                let ty_val = match arg_val {
                    ComptimeVal::Int(_) => Type::I64,
                    ComptimeVal::Float(_) => Type::F64,
                    ComptimeVal::Bool(_) => Type::Bool,
                    ComptimeVal::Str(_) => Type::Str,
                    ComptimeVal::Type(_) => Type::TypeVal,
                    ComptimeVal::Struct(name, _) => Type::Struct(name),
                    _ => Type::Void,
                };
                Ok(ComptimeVal::Type(ty_val))
            }
            "structinfo" => {
                if args.len() != 1 {
                    return Err(format!("@structinfo expects 1 argument, found {}", args.len()));
                }
                let struct_name = match &args[0] {
                    crate::hir::Expr::Ident(name) => {
                        if let Ok(Type::Struct(mangled)) = ctx.resolve_struct_type(name) {
                            mangled
                        } else {
                            let val = eval_comptime_expr(&args[0], env, ctx)?;
                            match val {
                                ComptimeVal::Type(Type::Struct(mangled)) => mangled,
                                ComptimeVal::Struct(mangled, _) => mangled,
                                _ => return Err(format!("@structinfo expects a struct type or expression")),
                            }
                        }
                    }
                    other => {
                        let val = eval_comptime_expr(other, env, ctx)?;
                        match val {
                            ComptimeVal::Type(Type::Struct(mangled)) => mangled,
                            ComptimeVal::Struct(mangled, _) => mangled,
                            _ => return Err(format!("@structinfo expects a struct type or expression")),
                        }
                    }
                };

                let fields = ctx.structs.get(&struct_name).ok_ok_or_else(|| {
                    format!("Struct '{}' fields not found in context", struct_name)
                })?.clone();

                let orig_name = if let Some(meta) = ctx.all_structs.iter().find(|s| {
                    mangle_name(&s.module_name, s.original_name, false) == struct_name
                }) {
                    meta.original_name
                } else {
                    &struct_name
                };

                let mut info_fields = HashMap::new();
                info_fields.insert("struct_name".to_string(), ComptimeVal::Str(orig_name.to_string()));
                for (f_name, f_ty) in fields {
                    info_fields.insert(f_name, ComptimeVal::Type(f_ty));
                }

                Ok(ComptimeVal::Struct(format!("StructInfo__{}", struct_name), info_fields))
            }
            other => Err(format!("Unknown compile-time builtin @{}", other)),
        }
        crate::hir::Expr::FieldAccess { expr, field } => {
            let base_val = eval_comptime_expr(expr, env, ctx)?;
            if let ComptimeVal::Struct(_, fields) = base_val {
                if let Some(val) = fields.get(*field) {
                    Ok(val.clone())
                } else {
                    Err(format!("Field '{}' not found on struct", field))
                }
            } else {
                Err(format!("Cannot access field '{}' on non-struct comptime value", field))
            }
        }
        crate::hir::Expr::StructLiteral { name, fields } => {
            let mut eval_fields = HashMap::new();
            for f in fields {
                let val = eval_comptime_expr(&f.value, env, ctx)?;
                eval_fields.insert(f.name.to_string(), val);
            }
            Ok(ComptimeVal::Struct(name.to_string(), eval_fields))
        }
        crate::hir::Expr::Call {
            name,
            type_args: _,
            args,
        } => {
            if *name == "puts" || *name == "std::io::puts" {
                if args.len() != 1 {
                    return Err("puts expects 1 argument".to_string());
                }
                let val = eval_comptime_expr(&args[0].value, env, ctx)?;
                match val {
                    ComptimeVal::Str(s) => {
                        println!("{}", s);
                        Ok(ComptimeVal::Int(s.len() as i64))
                    }
                    other => Err(format!("puts expects a string, found {:?}", other)),
                }
            } else {
                Err(format!(
                    "Calling function '{}' is not supported at compile-time",
                    name
                ))
            }
        }
        _ => Err("Expression not supported at compile time".to_string()),
    }
}

pub fn eval_comptime_block(
    block: &crate::hir::Block,
    env: &mut HashMap<String, ComptimeVal>,
    ctx: &SemaContext,
) -> Result<(), String> {
    for stmt in &block.statements {
        match stmt {
            crate::hir::Statement::Let {
                name,
                is_mut: _,
                value,
            } => {
                let val = eval_comptime_expr(value, env, ctx)?;
                env.insert(name.to_string(), val);
            }
            crate::hir::Statement::Assign {
                name,
                is_deref: _,
                value,
            } => {
                if env.contains_key(*name) {
                    let val = eval_comptime_expr(value, env, ctx)?;
                    env.insert(name.to_string(), val);
                } else {
                    return Err(format!("Variable '{}' not declared in comptime scope", name));
                }
            }
            crate::hir::Statement::Expr(expr) => {
                eval_comptime_expr(expr, env, ctx)?;
            }
            crate::hir::Statement::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_val = eval_comptime_expr(cond, env, ctx)?;
                match cond_val {
                    ComptimeVal::Bool(true) => {
                        eval_comptime_block(then_block, env, ctx)?;
                    }
                    ComptimeVal::Bool(false) => {
                        if let Some(eb) = else_block {
                            eval_comptime_block(eb, env, ctx)?;
                        }
                    }
                    other => {
                        return Err(format!(
                            "If condition must be a boolean expression, found {:?}",
                            other
                        ))
                    }
                }
            }
            crate::hir::Statement::While { cond, body } => loop {
                let cond_val = eval_comptime_expr(cond, env, ctx)?;
                match cond_val {
                    ComptimeVal::Bool(true) => {
                        eval_comptime_block(body, env, ctx)?;
                    }
                    ComptimeVal::Bool(false) => {
                        break;
                    }
                    other => {
                        return Err(format!(
                            "While condition must be a boolean expression, found {:?}",
                            other
                        ))
                    }
                }
            },
            _ => {}
        }
    }
    Ok(())
}
