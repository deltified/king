pub mod ast {
    use crate::parser::{BinOp, UnOp};
    use crate::hir::HirType;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Type {
        I64,
        F64,
        Bool,
        Void,
        Ref {
            is_mut: bool,
            ty: Box<Type>,
        },
        Struct(String),
    }

    impl From<HirType> for Type {
        fn from(ht: HirType) -> Self {
            match ht {
                HirType::I64 => Type::I64,
                HirType::F64 => Type::F64,
                HirType::Bool => Type::Bool,
                HirType::Void => Type::Void,
                HirType::Ref { is_mut, ty } => Type::Ref {
                    is_mut,
                    ty: Box::new(Type::from(*ty)),
                },
                HirType::Struct(name) => Type::Struct(name),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Program<'a> {
        pub structs: Vec<StructDef<'a>>,
        pub functions: Vec<Function<'a>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct StructDef<'a> {
        pub name: &'a str,
        pub fields: Vec<Param<'a>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Param<'a> {
        pub name: &'a str,
        pub ty: Type,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct FieldInit<'a> {
        pub name: &'a str,
        pub value: TypedExpr<'a>,
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
            is_deref: bool,
            value: TypedExpr<'a>,
        },
        AssignField {
            expr: TypedExpr<'a>,
            field: &'a str,
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
        Break,
        Continue,
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
        Float(f64),
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
        Call {
            name: &'a str,
            args: Vec<TypedExpr<'a>>,
        },
        As {
            expr: Box<TypedExpr<'a>>,
            ty: Type,
        },
        Borrow {
            is_mut: bool,
            expr: Box<TypedExpr<'a>>,
        },
        Deref(Box<TypedExpr<'a>>),
        StructLiteral {
            name: &'a str,
            fields: Vec<FieldInit<'a>>,
        },
        FieldAccess {
            expr: Box<TypedExpr<'a>>,
            field: &'a str,
        },
    }
}

pub use ast::*;

use std::collections::HashMap;

pub struct SemaContext<'a> {
    scopes: Vec<HashMap<&'a str, (Type, bool)>>,
    functions: HashMap<&'a str, (Vec<Type>, Type)>,
    pub structs: HashMap<String, Vec<(String, Type)>>,
    current_ret_type: Option<Type>,
    loop_depth: usize,
}

impl<'a> SemaContext<'a> {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            structs: HashMap::new(),
            current_ret_type: None,
            loop_depth: 0,
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
}

pub fn analyze<'a>(program: crate::hir::Program<'a>) -> Result<Program<'a>, String> {
    let mut ctx = SemaContext::new();
    
    // Register all structs first
    let mut structs = Vec::new();
    for s in &program.structs {
        let mut fields = Vec::new();
        let mut sema_fields = Vec::new();
        for f in &s.fields {
            let f_ty = Type::from(f.ty.clone());
            fields.push((f.name.to_string(), f_ty.clone()));
            sema_fields.push(Param { name: f.name, ty: f_ty });
        }
        ctx.structs.insert(s.name.to_string(), fields);
        structs.push(StructDef { name: s.name, fields: sema_fields });
    }

    for f in &program.functions {
        let param_tys: Vec<Type> = f.params.iter().map(|p| Type::from(p.ty.clone())).collect();
        ctx.functions.insert(f.name, (param_tys, Type::from(f.ret_type.clone())));
    }

    let mut functions = Vec::new();
    for f in program.functions {
        ctx.push_scope();
        ctx.current_ret_type = Some(Type::from(f.ret_type.clone()));

        let mut params = Vec::new();
        for p in &f.params {
            let ty = Type::from(p.ty.clone());
            ctx.declare_var(p.name, ty.clone(), true);
            params.push(Param { name: p.name, ty });
        }

        let body = check_block(&mut ctx, f.body)?;
        ctx.pop_scope();
        ctx.current_ret_type = None;

        functions.push(Function {
            name: f.name,
            params,
            ret_type: Type::from(f.ret_type.clone()),
            body,
        });
    }

    Ok(Program { structs, functions })
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
            ctx.declare_var(name, typed_value.ty.clone(), is_mut);
            Ok(Statement::Let { name, is_mut, value: typed_value })
        }
        crate::hir::Statement::Assign { name, is_deref, value } => {
            let (expected_ty, is_mut) = ctx.lookup_var(name)
                .ok_ok_or_else(|| format!("Variable '{}' not declared in scope", name))?;
            let typed_value = check_expr(ctx, value)?;
            if is_deref {
                match expected_ty {
                    Type::Ref { is_mut: ref_is_mut, ty: ref_inner_ty } => {
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
                        return Err(format!("Cannot dereference non-reference variable '{}' of type {:?}", name, expected_ty));
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
            Ok(Statement::Assign { name, is_deref, value: typed_value })
        }
        crate::hir::Statement::AssignField { expr, field, value } => {
            let checked_expr = check_expr(ctx, expr)?;
            if !is_writable(ctx, &checked_expr) {
                return Err(format!("Cannot assign to field '{}' of immutable expression", field));
            }

            let mut current_ty = &checked_expr.ty;
            while let Type::Ref { ty, .. } = current_ty {
                current_ty = ty;
            }

            match current_ty {
                Type::Struct(struct_name) => {
                    let field_ty = {
                        let struct_fields = ctx.structs.get(struct_name)
                            .ok_ok_or_else(|| format!("Struct '{}' not defined", struct_name))?;
                        let (_, field_ty) = struct_fields.iter().find(|(n, _)| n == field)
                            .ok_ok_or_else(|| format!("No field '{}' on struct '{}'", field, struct_name))?;
                        field_ty.clone()
                    };
                    
                    let checked_val = check_expr(ctx, value)?;
                    if checked_val.ty != field_ty {
                        return Err(format!("Type mismatch in field assignment for '{}': expected {:?}, found {:?}", field, field_ty, checked_val.ty));
                    }
                    
                    Ok(Statement::AssignField {
                        expr: checked_expr,
                        field,
                        value: checked_val,
                    })
                }
                other => {
                    Err(format!("Cannot assign to field '{}' of non-struct type {:?}", field, other))
                }
            }
        }
        crate::hir::Statement::Expr(expr) => {
            let typed_expr = check_expr(ctx, expr)?;
            Ok(Statement::Expr(typed_expr))
        }
        crate::hir::Statement::Return(opt_expr) => {
            let opt_typed_expr = opt_expr.map(|e| check_expr(ctx, e)).transpose()?;
            let found_ty = opt_typed_expr.as_ref().map(|e| e.ty.clone()).unwrap_or(Type::Void);
            let expected_ty = ctx.current_ret_type.clone().unwrap_or(Type::Void);
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
            ctx.loop_depth += 1;
            let typed_body = check_block(ctx, body);
            ctx.loop_depth -= 1;
            Ok(Statement::While { cond: typed_cond, body: typed_body? })
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
    }
}

fn check_expr<'a>(ctx: &mut SemaContext<'a>, expr: crate::hir::Expr<'a>) -> Result<TypedExpr<'a>, String> {
    match expr {
        crate::hir::Expr::Ident(name) => {
            let (ty, _) = ctx.lookup_var(name)
                .ok_ok_or_else(|| format!("Variable '{}' not found in scope", name))?;
            Ok(TypedExpr { kind: ExprKind::Ident(name), ty })
        }
        crate::hir::Expr::Int(val) => {
            Ok(TypedExpr { kind: ExprKind::Int(val), ty: Type::I64 })
        }
        crate::hir::Expr::Float(val) => {
            Ok(TypedExpr { kind: ExprKind::Float(val), ty: Type::F64 })
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
                    if typed_expr.ty != Type::I64 && typed_expr.ty != Type::F64 {
                        return Err(format!("Unary '-' operator cannot be applied to {:?}", typed_expr.ty));
                    }
                    typed_expr.ty.clone()
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
                    if (typed_lhs.ty == Type::I64 && typed_rhs.ty == Type::I64) ||
                       (typed_lhs.ty == Type::F64 && typed_rhs.ty == Type::F64) {
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
        crate::hir::Expr::Call { name, args } => {
            let (param_tys, ret_ty) = ctx.functions.get(name)
                .ok_ok_or_else(|| format!("Function '{}' not found in scope", name))?.clone();
            
            if args.len() != param_tys.len() {
                return Err(format!("Function '{}' expects {} arguments, found {}", name, param_tys.len(), args.len()));
            }
 
            let mut typed_args = Vec::new();
            for (arg, expected_ty) in args.into_iter().zip(param_tys) {
                let typed_arg = check_expr(ctx, arg)?;
                if typed_arg.ty != expected_ty {
                    return Err(format!("Argument type mismatch for function '{}': expected {:?}, found {:?}", name, expected_ty, typed_arg.ty));
                }
                typed_args.push(typed_arg);
            }
 
            Ok(TypedExpr {
                kind: ExprKind::Call { name, args: typed_args },
                ty: ret_ty,
            })
        }
        crate::hir::Expr::As { expr, ty } => {
            let typed_expr = check_expr(ctx, *expr)?;
            let dest_ty = Type::from(ty);
            
            // Check casting validity
            let valid = match (&typed_expr.ty, &dest_ty) {
                (Type::I64, Type::F64) | (Type::F64, Type::I64) => true,
                (t1, t2) if t1 == t2 => true,
                _ => false,
            };
 
            if !valid {
                return Err(format!("Cannot cast expression from {:?} to {:?}", typed_expr.ty, dest_ty));
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
                other => {
                    Err(format!("Cannot dereference non-reference type {:?}", other))
                }
            }
        }
        crate::hir::Expr::StructLiteral { name, fields } => {
            let struct_fields = ctx.structs.get(name)
                .ok_ok_or_else(|| format!("Struct '{}' not defined", name))?.clone();
            
            if fields.len() != struct_fields.len() {
                return Err(format!("Struct '{}' expects {} fields, found {}", name, struct_fields.len(), fields.len()));
            }
            
            let mut checked_fields = Vec::new();
            for (expected_name, expected_ty) in &struct_fields {
                let init = fields.iter().find(|f| f.name == expected_name)
                    .ok_ok_or_else(|| format!("Missing field '{}' in initializer for '{}'", expected_name, name))?;
                let checked_val = check_expr(ctx, init.value.clone())?;
                if checked_val.ty != *expected_ty {
                    return Err(format!("Type mismatch for field '{}' of struct '{}': expected {:?}, found {:?}", expected_name, name, expected_ty, checked_val.ty));
                }
                checked_fields.push(FieldInit { name: init.name, value: checked_val });
            }
            
            Ok(TypedExpr {
                kind: ExprKind::StructLiteral { name, fields: checked_fields },
                ty: Type::Struct(name.to_string()),
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
                    let struct_fields = ctx.structs.get(struct_name)
                        .ok_ok_or_else(|| format!("Struct '{}' not defined", struct_name))?;
                    let (_, field_ty) = struct_fields.iter().find(|(n, _)| n == field)
                        .ok_ok_or_else(|| format!("No field '{}' on struct '{}'", field, struct_name))?;
                    
                    Ok(TypedExpr {
                        kind: ExprKind::FieldAccess { expr: Box::new(checked_expr), field },
                        ty: field_ty.clone(),
                    })
                }
                other => {
                    Err(format!("Cannot access field '{}' on non-struct type {:?}", field, other))
                }
            }
        }
    }
}

fn is_writable<'a>(ctx: &SemaContext<'a>, expr: &TypedExpr<'a>) -> bool {
    match &expr.kind {
        ExprKind::Ident(name) => {
            if let Some((_, is_mut)) = ctx.lookup_var(name) {
                is_mut
            } else {
                false
            }
        }
        ExprKind::Deref(sub_expr) => {
            match &sub_expr.ty {
                Type::Ref { is_mut, .. } => *is_mut,
                _ => false,
            }
        }
        ExprKind::FieldAccess { expr: base_expr, .. } => {
            match &base_expr.ty {
                Type::Ref { is_mut, .. } => *is_mut,
                _ => is_writable(ctx, base_expr),
            }
        }
        _ => false,
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
