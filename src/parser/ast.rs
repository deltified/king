#[derive(Debug, PartialEq, Clone)]
pub struct Program<'a> {
    pub statements: Vec<Statement<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Type<'a> {
    Ident(&'a str),
    Ref {
        is_mut: bool,
        ty: Box<Type<'a>>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub struct Param<'a> {
    pub name: &'a str,
    pub ty: Type<'a>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct FieldDef<'a> {
    pub name: &'a str,
    pub ty: Type<'a>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct FieldInit<'a> {
    pub name: &'a str,
    pub value: Expr<'a>,
}

#[derive(Debug, PartialEq, Clone)]
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
    Function {
        name: &'a str,
        params: Vec<Param<'a>>,
        ret_type: Option<Type<'a>>,
        body: Vec<Statement<'a>>,
        is_pub: bool,
    },
    ExternFunction {
        name: &'a str,
        params: Vec<Param<'a>>,
        ret_type: Option<Type<'a>>,
    },
    StructDef {
        name: &'a str,
        fields: Vec<FieldDef<'a>>,
        is_pub: bool,
    },
    Import(Vec<&'a str>),
    Return(Option<Expr<'a>>),
    If {
        cond: Expr<'a>,
        then_block: Vec<Statement<'a>>,
        else_block: Option<Vec<Statement<'a>>>,
    },
    While {
        cond: Expr<'a>,
        body: Vec<Statement<'a>>,
    },
    Break,
    Continue,
}

#[derive(Debug, PartialEq, Clone)]
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
        ty: Type<'a>,
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

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnOp {
    Not,
    Neg,
}
