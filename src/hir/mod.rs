pub mod ast {
    use crate::parser::{BinOp, UnOp};

    #[derive(Debug, Clone, PartialEq)]
    pub struct Program<'a> {
        pub structs: Vec<StructDef<'a>>,
        pub functions: Vec<Function<'a>>,
        pub extern_functions: Vec<ExternFunction<'a>>,
        pub imports: std::collections::HashMap<String, Vec<String>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Param<'a> {
        pub name: &'a str,
        pub ty: HirType,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum HirType {
        I64,
        F64,
        Bool,
        Void,
        Char,
        Str,
        Ref {
            is_mut: bool,
            ty: Box<HirType>,
        },
        Struct(String),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct FieldDef<'a> {
        pub name: &'a str,
        pub ty: HirType,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct StructDef<'a> {
        pub name: &'a str,
        pub fields: Vec<FieldDef<'a>>,
        pub module_name: String,
        pub is_pub: bool,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct FieldInit<'a> {
        pub name: &'a str,
        pub value: Expr<'a>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Function<'a> {
        pub name: &'a str,
        pub params: Vec<Param<'a>>,
        pub ret_type: HirType,
        pub body: Block<'a>,
        pub module_name: String,
        pub is_pub: bool,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ExternFunction<'a> {
        pub name: &'a str,
        pub params: Vec<Param<'a>>,
        pub ret_type: HirType,
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
            value: Expr<'a>,
        },
        Assign {
            name: &'a str,
            is_deref: bool,
            value: Expr<'a>,
        },
        AssignField {
            expr: Expr<'a>,
            field: &'a str,
            value: Expr<'a>,
        },
        Expr(Expr<'a>),
        Return(Option<Expr<'a>>),
        If {
            cond: Expr<'a>,
            then_block: Block<'a>,
            else_block: Option<Block<'a>>,
        },
        While {
            cond: Expr<'a>,
            body: Block<'a>,
        },
        Break,
        Continue,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Expr<'a> {
        Ident(&'a str),
        Int(i64),
        Float(f64),
        Bool(bool),
        Str(String),
        Binary {
            op: BinOp,
            lhs: Box<Expr<'a>>,
            rhs: Box<Expr<'a>>,
        },
        Unary {
            op: UnOp,
            expr: Box<Expr<'a>>,
        },
        Call {
            name: &'a str,
            args: Vec<Expr<'a>>,
        },
        As {
            expr: Box<Expr<'a>>,
            ty: HirType,
        },
        Borrow {
            is_mut: bool,
            expr: Box<Expr<'a>>,
        },
        Deref(Box<Expr<'a>>),
        StructLiteral {
            name: &'a str,
            fields: Vec<FieldInit<'a>>,
        },
        FieldAccess {
            expr: Box<Expr<'a>>,
            field: &'a str,
        },
    }
}

pub use ast::*;

pub fn build<'a>(program: crate::parser::Program<'a>, module_name: &str) -> Program<'a> {
    let mut structs = Vec::new();
    let mut functions = Vec::new();
    let mut extern_functions = Vec::new();

    for stmt in program.statements {
        match stmt {
            crate::parser::Statement::Function { name, params, ret_type, body, is_pub } => {
                let params = params.into_iter().map(|p| Param {
                    name: p.name,
                    ty: lower_type(p.ty),
                }).collect();
                let ret_type = ret_type.map(lower_type).unwrap_or(HirType::Void);
                let body = build_block(body);
                functions.push(Function {
                    name,
                    params,
                    ret_type,
                    body,
                    module_name: module_name.to_string(),
                    is_pub,
                });
            }
            crate::parser::Statement::ExternFunction { name, params, ret_type } => {
                let params = params.into_iter().map(|p| Param {
                    name: p.name,
                    ty: lower_type(p.ty),
                }).collect();
                let ret_type = ret_type.map(lower_type).unwrap_or(HirType::Void);
                extern_functions.push(ExternFunction {
                    name,
                    params,
                    ret_type,
                });
            }
            crate::parser::Statement::StructDef { name, fields, is_pub } => {
                let fields = fields.into_iter().map(|f| FieldDef {
                    name: f.name,
                    ty: lower_type(f.ty),
                }).collect();
                structs.push(StructDef {
                    name,
                    fields,
                    module_name: module_name.to_string(),
                    is_pub,
                });
            }
            _ => {}
        }
    }
    Program { structs, functions, extern_functions, imports: std::collections::HashMap::new() }
}

fn lower_type(ty: crate::parser::Type) -> HirType {
    match ty {
        crate::parser::Type::Ident(name) => match name {
            "i64" => HirType::I64,
            "f64" => HirType::F64,
            "bool" => HirType::Bool,
            "char" => HirType::Char,
            "str" => HirType::Str,
            other => HirType::Struct(other.to_string()),
        },
        crate::parser::Type::Ref { is_mut, ty } => HirType::Ref {
            is_mut,
            ty: Box::new(lower_type(*ty)),
        },
    }
}

fn build_block<'a>(stmts: Vec<crate::parser::Statement<'a>>) -> Block<'a> {
    let statements = stmts.into_iter().map(build_statement).collect();
    Block { statements }
}

fn build_statement<'a>(stmt: crate::parser::Statement<'a>) -> Statement<'a> {
    match stmt {
        crate::parser::Statement::Let { name, is_mut, value } => Statement::Let {
            name,
            is_mut,
            value: build_expr(value),
        },
        crate::parser::Statement::Assign { name, is_deref, value } => Statement::Assign {
            name,
            is_deref,
            value: build_expr(value),
        },
        crate::parser::Statement::AssignField { expr, field, value } => Statement::AssignField {
            expr: build_expr(expr),
            field,
            value: build_expr(value),
        },
        crate::parser::Statement::Expr(expr) => Statement::Expr(build_expr(expr)),
        crate::parser::Statement::Return(opt_expr) => Statement::Return(opt_expr.map(build_expr)),
        crate::parser::Statement::If { cond, then_block, else_block } => Statement::If {
            cond: build_expr(cond),
            then_block: build_block(then_block),
            else_block: else_block.map(build_block),
        },
        crate::parser::Statement::While { cond, body } => Statement::While {
            cond: build_expr(cond),
            body: build_block(body),
        },
        crate::parser::Statement::Break => Statement::Break,
        crate::parser::Statement::Continue => Statement::Continue,
        crate::parser::Statement::StructDef { .. }
        | crate::parser::Statement::Function { .. }
        | crate::parser::Statement::ExternFunction { .. }
        | crate::parser::Statement::Import(_) => {
            panic!("Nested items not supported in HIR builder");
        }
    }
}

fn build_expr<'a>(expr: crate::parser::Expr<'a>) -> Expr<'a> {
    match expr {
        crate::parser::Expr::Ident(name) => Expr::Ident(name),
        crate::parser::Expr::Int(val) => Expr::Int(val),
        crate::parser::Expr::Float(val) => Expr::Float(val),
        crate::parser::Expr::Bool(val) => Expr::Bool(val),
        crate::parser::Expr::Str(val) => Expr::Str(val),
        crate::parser::Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op,
            lhs: Box::new(build_expr(*lhs)),
            rhs: Box::new(build_expr(*rhs)),
        },
        crate::parser::Expr::Unary { op, expr } => Expr::Unary {
            op,
            expr: Box::new(build_expr(*expr)),
        },
        crate::parser::Expr::Call { name, args } => Expr::Call {
            name,
            args: args.into_iter().map(build_expr).collect(),
        },
        crate::parser::Expr::As { expr, ty } => Expr::As {
            expr: Box::new(build_expr(*expr)),
            ty: lower_type(ty),
        },
        crate::parser::Expr::Borrow { is_mut, expr } => Expr::Borrow {
            is_mut,
            expr: Box::new(build_expr(*expr)),
        },
        crate::parser::Expr::Deref(expr) => Expr::Deref(Box::new(build_expr(*expr))),
        crate::parser::Expr::StructLiteral { name, fields } => {
            let fields = fields.into_iter().map(|f| FieldInit {
                name: f.name,
                value: build_expr(f.value),
            }).collect();
            Expr::StructLiteral { name, fields }
        }
        crate::parser::Expr::FieldAccess { expr, field } => Expr::FieldAccess {
            expr: Box::new(build_expr(*expr)),
            field,
        },
    }
}
