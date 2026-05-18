pub mod ast {
    use crate::parser::{BinOp, UnOp};

    #[derive(Debug, Clone, PartialEq)]
    pub struct Program<'a> {
        pub structs: Vec<StructDef<'a>>,
        pub functions: Vec<Function<'a>>,
        pub extern_functions: Vec<ExternFunction<'a>>,
        pub traits: Vec<TraitDef<'a>>,
        pub impls: Vec<ImplDef<'a>>,
        pub imports: std::collections::HashMap<String, Vec<String>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Param<'a> {
        pub name: &'a str,
        pub ty: HirType,
        pub contract: Option<Expr<'a>>,
        pub default: Option<Expr<'a>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct CallArg<'a> {
        pub name: Option<&'a str>,
        pub value: Expr<'a>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum HirType {
        I64,
        F64,
        Bool,
        Void,
        Char,
        Str,
        TypeVal,
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
        pub generics: Vec<&'a str>,
        pub generic_contracts: Vec<Option<Expr<'a>>>,
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
        pub module_name: String,
        pub is_pub: bool,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct TraitMethod<'a> {
        pub name: &'a str,
        pub params: Vec<Param<'a>>,
        pub ret_type: HirType,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct TraitDef<'a> {
        pub name: &'a str,
        pub methods: Vec<TraitMethod<'a>>,
        pub module_name: String,
        pub is_pub: bool,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ImplDef<'a> {
        pub trait_name: &'a str,
        pub for_types: Vec<HirType>,
        pub methods: Vec<Function<'a>>,
        pub module_name: String,
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
        AssertLet {
            name: &'a str,
            is_mut: bool,
            value: Expr<'a>,
        },
        HandleLet {
            name: &'a str,
            is_mut: bool,
            value: Expr<'a>,
            ok_body: Block<'a>,
            err_body: Block<'a>,
            is_ok_escape: bool,
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
        Comptime(Block<'a>),
        InlineFor {
            var_name: &'a str,
            start: Expr<'a>,
            end: Expr<'a>,
            body: Block<'a>,
        },
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
            type_args: Vec<HirType>,
            args: Vec<CallArg<'a>>,
        },
        MethodCall {
            expr: Box<Expr<'a>>,
            method: &'a str,
            args: Vec<CallArg<'a>>,
        },
        As {
            expr: Box<Expr<'a>>,
            ty: HirType,
        },
        Is {
            expr: Box<Expr<'a>>,
            ty: HirType,
        },
        BuiltinCall {
            name: &'a str,
            args: Vec<Expr<'a>>,
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
        IndexAccess {
            expr: Box<Expr<'a>>,
            index: Box<Expr<'a>>,
        },
        New(Box<Expr<'a>>),
    }
}

pub use ast::*;

pub fn build<'a>(program: crate::parser::Program<'a>, module_name: &str) -> Program<'a> {
    let mut structs = Vec::new();
    let mut functions = Vec::new();
    let mut extern_functions = Vec::new();
    let mut traits = Vec::new();
    let mut impls = Vec::new();

    for stmt in program.statements {
        match stmt {
            crate::parser::Statement::Function { name, generics, generic_contracts, params, ret_type, body, is_pub } => {
                let params = params.into_iter().map(|p| Param {
                    name: p.name,
                    ty: lower_type(p.ty),
                    contract: p.contract.map(build_expr),
                    default: p.default.map(build_expr),
                }).collect();
                let ret_type = ret_type.map(lower_type).unwrap_or(HirType::Void);
                let body = build_block(body);
                functions.push(Function {
                    name,
                    generics,
                    generic_contracts: generic_contracts.into_iter().map(|opt| opt.map(build_expr)).collect(),
                    params,
                    ret_type,
                    body,
                    module_name: module_name.to_string(),
                    is_pub,
                });
            }
            crate::parser::Statement::ExternFunction { name, params, ret_type, is_pub } => {
                let params = params.into_iter().map(|p| Param {
                    name: p.name,
                    ty: lower_type(p.ty),
                    contract: p.contract.map(build_expr),
                    default: p.default.map(build_expr),
                }).collect();
                let ret_type = ret_type.map(lower_type).unwrap_or(HirType::Void);
                extern_functions.push(ExternFunction {
                    name,
                    params,
                    ret_type,
                    module_name: module_name.to_string(),
                    is_pub,
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
            crate::parser::Statement::TraitDef { name, methods, is_pub } => {
                let lowered_methods = methods.into_iter().map(|m| TraitMethod {
                    name: m.name,
                    params: m.params.into_iter().map(|p| Param {
                        name: p.name,
                        ty: lower_type(p.ty),
                        contract: p.contract.map(build_expr),
                        default: p.default.map(build_expr),
                    }).collect(),
                    ret_type: m.ret_type.map(lower_type).unwrap_or(HirType::Void),
                }).collect();
                traits.push(TraitDef {
                    name,
                    methods: lowered_methods,
                    module_name: module_name.to_string(),
                    is_pub,
                });
            }
            crate::parser::Statement::ImplDef { trait_name, for_types, methods } => {
                let lowered_methods = methods.into_iter().map(|m| {
                    match m {
                        crate::parser::Statement::Function { name, generics, generic_contracts, params, ret_type, body, is_pub } => {
                            let params = params.into_iter().map(|p| Param {
                                name: p.name,
                                ty: lower_type(p.ty),
                                contract: p.contract.map(build_expr),
                                default: p.default.map(build_expr),
                            }).collect();
                            let ret_type = ret_type.map(lower_type).unwrap_or(HirType::Void);
                            let body = build_block(body);
                            Function {
                                name,
                                generics,
                                generic_contracts: generic_contracts.into_iter().map(|opt| opt.map(build_expr)).collect(),
                                params,
                                ret_type,
                                body,
                                module_name: module_name.to_string(),
                                is_pub,
                            }
                        }
                        _ => panic!("Expected function inside impl block"),
                    }
                }).collect();
                let lowered_for_types = for_types.into_iter().map(lower_type).collect();
                impls.push(ImplDef {
                    trait_name,
                    for_types: lowered_for_types,
                    methods: lowered_methods,
                    module_name: module_name.to_string(),
                });
            }
            _ => {}
        }
    }
    Program { structs, functions, extern_functions, traits, impls, imports: std::collections::HashMap::new() }
}

fn lower_type(ty: crate::parser::Type) -> HirType {
    match ty {
        crate::parser::Type::Ident(name) => match name {
            "i64" => HirType::I64,
            "f64" => HirType::F64,
            "bool" => HirType::Bool,
            "char" => HirType::Char,
            "str" => HirType::Str,
            "type" => HirType::TypeVal,
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
        crate::parser::Statement::AssertLet { name, is_mut, value } => Statement::AssertLet {
            name,
            is_mut,
            value: build_expr(value),
        },
        crate::parser::Statement::HandleLet { name, is_mut, value, ok_body, err_body, is_ok_escape } => Statement::HandleLet {
            name,
            is_mut,
            value: build_expr(value),
            ok_body: build_block(ok_body),
            err_body: build_block(err_body),
            is_ok_escape,
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
        crate::parser::Statement::Comptime(body) => Statement::Comptime(build_block(body)),
        crate::parser::Statement::InlineFor { var_name, start, end, body } => Statement::InlineFor {
            var_name,
            start: build_expr(start),
            end: build_expr(end),
            body: build_block(body),
        },
        crate::parser::Statement::StructDef { .. }
        | crate::parser::Statement::Function { .. }
        | crate::parser::Statement::ExternFunction { .. }
        | crate::parser::Statement::TraitDef { .. }
        | crate::parser::Statement::ImplDef { .. }
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
        crate::parser::Expr::Call { name, type_args, args } => Expr::Call {
            name,
            type_args: type_args.into_iter().map(lower_type).collect(),
            args: args.into_iter().map(|arg| CallArg {
                name: arg.name,
                value: build_expr(arg.value),
            }).collect(),
        },
        crate::parser::Expr::MethodCall { expr, method, args } => Expr::MethodCall {
            expr: Box::new(build_expr(*expr)),
            method,
            args: args.into_iter().map(|arg| CallArg {
                name: arg.name,
                value: build_expr(arg.value),
            }).collect(),
        },
        crate::parser::Expr::As { expr, ty } => Expr::As {
            expr: Box::new(build_expr(*expr)),
            ty: lower_type(ty),
        },
        crate::parser::Expr::Is { expr, ty } => Expr::Is {
            expr: Box::new(build_expr(*expr)),
            ty: lower_type(ty),
        },
        crate::parser::Expr::BuiltinCall { name, args } => Expr::BuiltinCall {
            name,
            args: args.into_iter().map(build_expr).collect(),
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
        crate::parser::Expr::IndexAccess { expr, index } => Expr::IndexAccess {
            expr: Box::new(build_expr(*expr)),
            index: Box::new(build_expr(*index)),
        },
        crate::parser::Expr::New(expr) => Expr::New(Box::new(build_expr(*expr))),
    }
}
