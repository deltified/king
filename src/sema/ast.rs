use crate::parser::{BinOp, UnOp};
use crate::hir::HirType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    I64,
    F64,
    Bool,
    Void,
    Char,
    Str,
    TypeVal,
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
            HirType::Char => Type::Char,
            HirType::Str => Type::Str,
            HirType::TypeVal => Type::TypeVal,
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
    pub extern_functions: Vec<ExternFunction<'a>>,
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
    pub contract: Option<TypedExpr<'a>>,
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
pub struct ExternFunction<'a> {
    pub name: &'a str,
    pub params: Vec<Param<'a>>,
    pub ret_type: Type,
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
    AssertLet {
        name: &'a str,
        is_mut: bool,
        value: TypedExpr<'a>,
    },
    HandleLet {
        name: &'a str,
        is_mut: bool,
        value: TypedExpr<'a>,
        ok_body: Block<'a>,
        err_body: Block<'a>,
        is_ok_escape: bool,
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
    Str(String),
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
