pub mod ast {
    use crate::parser::{BinOp, UnOp};
    use crate::hir::HirType;

    #[derive(Debug, Clone, PartialEq, Eq, Copy)]
    pub enum Type {
        I64,
        Bool,
        Void,
    }

    impl From<HirType> for Type {
        fn from(ht: HirType) -> Self {
            match ht {
                HirType::I64 => Type::I64,
                HirType::Bool => Type::Bool,
                HirType::Void => Type::Void,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Program<'a> {
        pub functions: Vec<Function<'a>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Param<'a> {
        pub name: &'a str,
        pub ty: Type,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Function<'a> {
        pub name: &'a str,
        pub params: Vec<Param<'a>>,
        pub ret_type: Type,
        pub body: Block<'a>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Block<'a> {
        pub statements: Vec<Statement<'a>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Statement<'a> {
        Let {
            name: &'a str,
            is_mut: bool,
            value: TypedExpr<'a>,
        },
        Assign {
            name: &'a str,
            value: TypedExpr<'a>,
        },
        Expr(TypedExpr<'a>),
        Return(Option<TypedExpr<'a>>),
        If {
            cond: TypedExpr<'a>,
            then_block: Block<'a>,
            else_block: Option<Block<'a>>,
        },
        While {
            cond: TypedExpr<'a>,
            body: Block<'a>,
        },
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct TypedExpr<'a> {
        pub kind: ExprKind<'a>,
        pub ty: Type,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum ExprKind<'a> {
        Ident(&'a str),
        Int(i64),
        Bool(bool),
        Binary {
            op: BinOp,
            lhs: Box<TypedExpr<'a>>,
            rhs: Box<TypedExpr<'a>>,
        },
        Unary {
            op: UnOp,
            expr: Box<TypedExpr<'a>>,
        },
    }
}

pub use ast::*;

use std::collections::HashMap;

pub struct SemaContext<'a> {
    scopes: Vec<HashMap<&'a str, Type>>,
    functions: HashMap<&'a str, (Vec<Type>, Type)>,
    current_ret_type: Option<Type>,
}

impl<'a> SemaContext<'a> {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            current_ret_type: None,
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn declare_var(&mut self, name: &'a str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    pub fn lookup_var(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(*ty);
            }
        }
        None
    }
}

pub fn analyze<'a>(program: crate::hir::Program<'a>) -> Result<Program<'a>, String> {
    let mut ctx = SemaContext::new();
    
    // Register all function signatures first
    for f in &program.functions {
        let param_tys: Vec<Type> = f.params.iter().map(|p| Type::from(p.ty)).collect();
        ctx.functions.insert(f.name, (param_tys, Type::from(f.ret_type)));
    }

    let mut functions = Vec::new();
    for f in program.functions {
        ctx.push_scope();
        ctx.current_ret_type = Some(Type::from(f.ret_type));

        let mut params = Vec::new();
        for p in &f.params {
            let ty = Type::from(p.ty);
            ctx.declare_var(p.name, ty);
            params.push(Param { name: p.name, ty });
        }

        let body = check_block(&mut ctx, f.body)?;
        ctx.pop_scope();
        ctx.current_ret_type = None;

        functions.push(Function {
            name: f.name,
            params,
            ret_type: Type::from(f.ret_type),
            body,
        });
    }

    Ok(Program { functions })
}

fn check_block<'a>(ctx: &mut SemaContext<'a>, block: crate::hir::Block<'a>) -> Result<Block<'a>, String> {
    ctx.push_scope();
    let mut statements = Vec::new();
    for stmt in block.statements {
        statements.push(check_statement(ctx, stmt)?);
    }
    ctx.pop_scope();
    Ok(Block { statements })
}

fn check_statement<'a>(ctx: &mut SemaContext<'a>, stmt: crate::hir::Statement<'a>) -> Result<Statement<'a>, String> {
    match stmt {
        crate::hir::Statement::Let { name, is_mut, value } => {
            let typed_value = check_expr(ctx, value)?;
            ctx.declare_var(name, typed_value.ty);
            Ok(Statement::Let { name, is_mut, value: typed_value })
        }
        crate::hir::Statement::Assign { name, value } => {
            let expected_ty = ctx.lookup_var(name)
                .ok_ok_or_else(|| format!("Variable '{}' not declared in scope", name))?;
            let typed_value = check_expr(ctx, value)?;
            if typed_value.ty != expected_ty {
                return Err(format!(
                    "Type mismatch in assignment for variable '{}': expected {:?}, found {:?}",
                    name, expected_ty, typed_value.ty
                ));
            }
            Ok(Statement::Assign { name, value: typed_value })
        }
        crate::hir::Statement::Expr(expr) => {
            let typed_expr = check_expr(ctx, expr)?;
            Ok(Statement::Expr(typed_expr))
        }
        crate::hir::Statement::Return(opt_expr) => {
            let opt_typed_expr = opt_expr.map(|e| check_expr(ctx, e)).transpose()?;
            let found_ty = opt_typed_expr.as_ref().map(|e| e.ty).unwrap_or(Type::Void);
            let expected_ty = ctx.current_ret_type.unwrap_or(Type::Void);
            if found_ty != expected_ty {
                return Err(format!(
                    "Return type mismatch: expected {:?}, found {:?}",
                    expected_ty, found_ty
                ));
            }
            Ok(Statement::Return(opt_typed_expr))
        }
        crate::hir::Statement::If { cond, then_block, else_block } => {
            let typed_cond = check_expr(ctx, cond)?;
            if typed_cond.ty != Type::Bool {
                return Err(format!("If condition must be a boolean expression, found {:?}", typed_cond.ty));
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
                return Err(format!("While loop condition must be a boolean expression, found {:?}", typed_cond.ty));
            }
            let typed_body = check_block(ctx, body)?;
            Ok(Statement::While { cond: typed_cond, body: typed_body })
        }
    }
}

fn check_expr<'a>(ctx: &mut SemaContext<'a>, expr: crate::hir::Expr<'a>) -> Result<TypedExpr<'a>, String> {
    match expr {
        crate::hir::Expr::Ident(name) => {
            let ty = ctx.lookup_var(name)
                .ok_ok_or_else(|| format!("Variable '{}' not found in scope", name))?;
            Ok(TypedExpr { kind: ExprKind::Ident(name), ty })
        }
        crate::hir::Expr::Int(val) => {
            Ok(TypedExpr { kind: ExprKind::Int(val), ty: Type::I64 })
        }
        crate::hir::Expr::Bool(val) => {
            Ok(TypedExpr { kind: ExprKind::Bool(val), ty: Type::Bool })
        }
        crate::hir::Expr::Unary { op, expr } => {
            use crate::parser::UnOp;
            let typed_expr = check_expr(ctx, *expr)?;
            let res_ty = match op {
                UnOp::Not => {
                    if typed_expr.ty != Type::Bool {
                        return Err(format!("Unary '!' operator cannot be applied to {:?}", typed_expr.ty));
                    }
                    Type::Bool
                }
                UnOp::Neg => {
                    if typed_expr.ty != Type::I64 {
                        return Err(format!("Unary '-' operator cannot be applied to {:?}", typed_expr.ty));
                    }
                    Type::I64
                }
            };
            Ok(TypedExpr {
                kind: ExprKind::Unary { op, expr: Box::new(typed_expr) },
                ty: res_ty,
            })
        }
        crate::hir::Expr::Binary { op, lhs, rhs } => {
            use crate::parser::BinOp;
            let typed_lhs = check_expr(ctx, *lhs)?;
            let typed_rhs = check_expr(ctx, *rhs)?;
            
            let res_ty = match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                    if typed_lhs.ty != Type::I64 || typed_rhs.ty != Type::I64 {
                        return Err(format!("Arithmetic operator {:?} requires I64 operands, found {:?} and {:?}", op, typed_lhs.ty, typed_rhs.ty));
                    }
                    Type::I64
                }
                BinOp::And | BinOp::Or => {
                    if typed_lhs.ty != Type::Bool || typed_rhs.ty != Type::Bool {
                        return Err(format!("Logical operator {:?} requires Bool operands, found {:?} and {:?}", op, typed_lhs.ty, typed_rhs.ty));
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
                    if typed_lhs.ty != Type::I64 || typed_rhs.ty != Type::I64 {
                        return Err(format!("Relational operator {:?} requires I64 operands, found {:?} and {:?}", op, typed_lhs.ty, typed_rhs.ty));
                    }
                    Type::Bool
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
    }
}

trait OptionExt<T> {
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
