use std::collections::HashMap;
use super::ast::{Block, ExprKind, Statement, Type, TypedExpr};
use super::context::{OptionExt, SemaContext};
use super::comptime::eval_comptime_block;
use super::expr::{check_expr, is_writable};

pub fn check_block<'a>(
    ctx: &mut SemaContext<'a>,
    block: crate::hir::Block<'a>,
) -> Result<Block<'a>, String> {
    ctx.push_scope();
    let mut statements = Vec::new();
    for stmt in block.statements {
        statements.push(check_statement(ctx, stmt)?);
    }
    ctx.pop_scope();
    Ok(Block { statements })
}

pub fn check_statement<'a>(
    ctx: &mut SemaContext<'a>,
    stmt: crate::hir::Statement<'a>,
) -> Result<Statement<'a>, String> {
    match stmt {
        crate::hir::Statement::Let {
            name,
            is_mut,
            value,
        } => {
            let typed_value = check_expr(ctx, value)?;
            ctx.declare_var(name, typed_value.ty.clone(), is_mut);
            Ok(Statement::Let {
                name,
                is_mut,
                value: typed_value,
            })
        }
        crate::hir::Statement::Assign {
            name,
            is_deref,
            value,
        } => {
            let (expected_ty, is_mut) = ctx
                .lookup_var(name)
                .ok_ok_or_else(|| format!("Variable '{}' not declared in scope", name))?;
            let typed_value = check_expr(ctx, value)?;
            if is_deref {
                match expected_ty {
                    Type::Ref {
                        is_mut: ref_is_mut,
                        ty: ref_inner_ty,
                    } => {
                        if !ref_is_mut {
                            return Err(format!("Cannot assign to immutable reference '{}'", name));
                        }
                        if typed_value.ty != *ref_inner_ty {
                            return Err(format!(
                                "Type mismatch in dereference assignment for variable '{}': expected {:?}, found {:?}",
                                name, ref_inner_ty, typed_value.ty
                            ));
                        }
                    }
                    _ => {
                        return Err(format!(
                            "Cannot dereference non-reference variable '{}' of type {:?}",
                            name, expected_ty
                        ));
                    }
                }
            } else {
                if !is_mut {
                    return Err(format!("Cannot reassign immutable variable '{}'", name));
                }
                if typed_value.ty != expected_ty {
                    return Err(format!(
                        "Type mismatch in assignment for variable '{}': expected {:?}, found {:?}",
                        name, expected_ty, typed_value.ty
                    ));
                }
            }
            Ok(Statement::Assign {
                name,
                is_deref,
                value: typed_value,
            })
        }
        crate::hir::Statement::AssignField { expr, field, value } => {
            let checked_expr = check_expr(ctx, expr)?;
            if !is_writable(ctx, &checked_expr) {
                return Err(format!(
                    "Cannot assign to field '{}' of immutable expression",
                    field
                ));
            }

            let mut current_ty = &checked_expr.ty;
            while let Type::Ref { ty, .. } = current_ty {
                current_ty = ty;
            }

            match current_ty {
                Type::Struct(struct_name) => {
                    let field_ty = {
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
                        field_ty.clone()
                    };

                    let checked_val = check_expr(ctx, value)?;
                    if checked_val.ty != field_ty {
                        return Err(format!(
                            "Type mismatch in field assignment for '{}': expected {:?}, found {:?}",
                            field, field_ty, checked_val.ty
                        ));
                    }

                    Ok(Statement::AssignField {
                        expr: checked_expr,
                        field,
                        value: checked_val,
                    })
                }
                other => Err(format!(
                    "Cannot assign to field '{}' of non-struct type {:?}",
                    field, other
                )),
            }
        }
        crate::hir::Statement::Expr(expr) => {
            let typed_expr = check_expr(ctx, expr)?;
            Ok(Statement::Expr(typed_expr))
        }
        crate::hir::Statement::Return(opt_expr) => {
            let opt_typed_expr = opt_expr.map(|e| check_expr(ctx, e)).transpose()?;
            let found_ty = opt_typed_expr
                .as_ref()
                .map(|e| e.ty.clone())
                .unwrap_or(Type::Void);
            let expected_ty = ctx.current_ret_type.clone().unwrap_or(Type::Void);
            if found_ty != expected_ty {
                return Err(format!(
                    "Return type mismatch: expected {:?}, found {:?}",
                    expected_ty, found_ty
                ));
            }
            Ok(Statement::Return(opt_typed_expr))
        }
        crate::hir::Statement::If {
            cond,
            then_block,
            else_block,
        } => {
            let typed_cond = check_expr(ctx, cond)?;
            if typed_cond.ty != Type::Bool {
                return Err(format!(
                    "If condition must be a boolean expression, found {:?}",
                    typed_cond.ty
                ));
            }
            let typed_then = check_block(ctx, then_block)?;
            let typed_else = else_block.map(|b| check_block(ctx, b)).transpose()?;
            Ok(Statement::If {
                cond: typed_cond,
                then_block: typed_then,
                else_block: typed_else,
            })
        }
        crate::hir::Statement::While { cond, body } => {
            let typed_cond = check_expr(ctx, cond)?;
            if typed_cond.ty != Type::Bool {
                return Err(format!(
                    "While loop condition must be a boolean expression, found {:?}",
                    typed_cond.ty
                ));
            }
            ctx.loop_depth += 1;
            let typed_body = check_block(ctx, body);
            ctx.loop_depth -= 1;
            Ok(Statement::While {
                cond: typed_cond,
                body: typed_body?,
            })
        }
        crate::hir::Statement::Break => {
            if ctx.loop_depth == 0 {
                return Err("break statement outside of a loop".to_string());
            }
            Ok(Statement::Break)
        }
        crate::hir::Statement::Continue => {
            if ctx.loop_depth == 0 {
                return Err("continue statement outside of a loop".to_string());
            }
            Ok(Statement::Continue)
        }
        crate::hir::Statement::Comptime(block) => {
            let mut env = HashMap::new();
            eval_comptime_block(&block, &mut env, ctx)?;
            Ok(Statement::Expr(TypedExpr {
                kind: ExprKind::Int(0),
                ty: Type::Void,
            }))
        }
    }
}
