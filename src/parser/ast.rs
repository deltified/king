#[derive(Debug, PartialEq, Clone)]
pub struct Program<'a> {
    pub statements: Vec<Statement<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Statement<'a> {
    Let {
        name: &'a str,
        value: Expr<'a>,
    },
    Assign {
        name: &'a str,
        value: Expr<'a>,
    },
    Expr(Expr<'a>),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr<'a> {
    Ident(&'a str),
    Int(i64),
    Binary {
        op: BinOp,
        lhs: Box<Expr<'a>>,
        rhs: Box<Expr<'a>>,
    },
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}
