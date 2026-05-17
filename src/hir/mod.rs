pub mod ast {
    use crate::parser::{BinOp, UnOp};

    #[derive(Debug, Clone, PartialEq)]
    pub struct Program<'a> {
        pub functions: Vec<Function<'a>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Param<'a> {
        pub name: &'a str,
        pub ty: HirType,
    }

    #[derive(Debug, Clone, PartialEq, Copy)]
    pub enum HirType {
        I64,
        F64,
        Bool,
        Void,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Function<'a> {
        pub name: &'a str,
        pub params: Vec<Param<'a>>,
        pub ret_type: HirType,
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
            value: Expr<'a>,
        },
        Assign {
            name: &'a str,
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
    }
}

pub use ast::*;

pub fn build<'a>(program: crate::parser::Program<'a>) -> Program<'a> {
    let mut functions = Vec::new();
    for stmt in program.statements {
        match stmt {
            crate::parser::Statement::Function { name, params, ret_type, body } => {
                let params = params.into_iter().map(|p| Param {
                    name: p.name,
                    ty: parse_type(p.ty),
                }).collect();
                let ret_type = ret_type.map(parse_type).unwrap_or(HirType::Void);
                let body = build_block(body);
                functions.push(Function { name, params, ret_type, body });
            }
            _ => {}
        }
    }
    Program { functions }
}

fn parse_type(ty: &str) -> HirType {
    match ty {
        "i64" => HirType::I64,
        "f64" => HirType::F64,
        "bool" => HirType::Bool,
        _ => HirType::Void,
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
        crate::parser::Statement::Assign { name, value } => Statement::Assign {
            name,
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
        crate::parser::Statement::Function { .. } => {
            panic!("Nested functions not supported in HIR builder");
        }
    }
}

fn build_expr<'a>(expr: crate::parser::Expr<'a>) -> Expr<'a> {
    match expr {
        crate::parser::Expr::Ident(name) => Expr::Ident(name),
        crate::parser::Expr::Int(val) => Expr::Int(val),
        crate::parser::Expr::Float(val) => Expr::Float(val),
        crate::parser::Expr::Bool(val) => Expr::Bool(val),
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
            ty: parse_type(ty),
        },
    }
}
