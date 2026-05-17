pub mod ast {
    use crate::sema::ast::Type;
    use crate::parser::{BinOp, UnOp};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct BasicBlockId(pub usize);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct VarId(pub usize);

    #[derive(Debug, Clone, PartialEq)]
    pub enum Operand<'a> {
        Var(VarId),
        Int(i64),
        Float(f64),
        Bool(bool),
        Ident(&'a str),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Rvalue<'a> {
        Use(Operand<'a>),
        Binary(BinOp, Operand<'a>, Operand<'a>),
        Unary(UnOp, Operand<'a>),
        Call(&'a str, Vec<Operand<'a>>),
        As(Operand<'a>, Type),
        Ref(bool, VarId),
        RefVar(bool, &'a str),
        Deref(Operand<'a>),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Statement<'a> {
        Assign(VarId, Rvalue<'a>),
        AssignVar(&'a str, Operand<'a>),
        Store(VarId, Operand<'a>),
        StoreVar(&'a str, Operand<'a>),
        Call(&'a str, Vec<Operand<'a>>),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Terminator<'a> {
        Return(Option<Operand<'a>>),
        Goto(BasicBlockId),
        CondBranch {
            cond: Operand<'a>,
            then_block: BasicBlockId,
            else_block: BasicBlockId,
        },
        Unreachable,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct BasicBlock<'a> {
        pub id: BasicBlockId,
        pub statements: Vec<Statement<'a>>,
        pub terminator: Terminator<'a>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Function<'a> {
        pub name: &'a str,
        pub params: Vec<(&'a str, Type)>,
        pub ret_type: Type,
        pub basic_blocks: Vec<BasicBlock<'a>>,
        pub vars: Vec<Type>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Program<'a> {
        pub functions: Vec<Function<'a>>,
    }
}

pub use ast::*;

use std::collections::HashMap;
use crate::sema::ast::Type;

struct MirBuilderContext<'a> {
    basic_blocks: Vec<BasicBlock<'a>>,
    current_block: Option<BasicBlockId>,
    vars: Vec<Type>,
    var_map: HashMap<&'a str, VarId>,
    next_block_id: usize,
    loop_stack: Vec<(BasicBlockId, BasicBlockId)>, // (continue_target, break_target)
}

impl<'a> MirBuilderContext<'a> {
    fn new() -> Self {
        Self {
            basic_blocks: Vec::new(),
            current_block: None,
            vars: Vec::new(),
            var_map: HashMap::new(),
            next_block_id: 0,
            loop_stack: Vec::new(),
        }
    }

    fn new_block(&mut self) -> BasicBlockId {
        let id = BasicBlockId(self.next_block_id);
        self.next_block_id += 1;
        self.basic_blocks.push(BasicBlock {
            id,
            statements: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        id
    }

    fn start_block(&mut self, id: BasicBlockId) {
        self.current_block = Some(id);
    }

    fn push_statement(&mut self, stmt: Statement<'a>) {
        if let Some(curr) = self.current_block {
            if let Some(bb) = self.basic_blocks.iter_mut().find(|b| b.id == curr) {
                bb.statements.push(stmt);
            }
        }
    }

    fn terminate(&mut self, term: Terminator<'a>) {
        if let Some(curr) = self.current_block {
            if let Some(bb) = self.basic_blocks.iter_mut().find(|b| b.id == curr) {
                bb.terminator = term;
            }
            self.current_block = None;
        }
    }

    fn declare_var(&mut self, name: &'a str, ty: Type) -> VarId {
        let id = VarId(self.vars.len());
        self.vars.push(ty);
        self.var_map.insert(name, id);
        id
    }

    fn declare_temp(&mut self, ty: Type) -> VarId {
        let id = VarId(self.vars.len());
        self.vars.push(ty);
        id
    }
}

pub fn build<'a>(program: crate::sema::Program<'a>) -> Program<'a> {
    let mut functions = Vec::new();
    for f in program.functions {
        let mut ctx = MirBuilderContext::new();
        
        let entry = ctx.new_block();
        ctx.start_block(entry);

        let params: Vec<(&'a str, Type)> = f.params.iter().map(|p| (p.name, p.ty.clone())).collect();

        compile_block(&mut ctx, f.body);

        // Ensure entry block or last block is terminated with standard return if no explicit return is present
        if let Some(_curr) = ctx.current_block {
            ctx.terminate(Terminator::Return(None));
        }

        functions.push(Function {
            name: f.name,
            params,
            ret_type: f.ret_type,
            basic_blocks: ctx.basic_blocks,
            vars: ctx.vars,
        });
    }
    Program { functions }
}

fn compile_block<'a>(ctx: &mut MirBuilderContext<'a>, block: crate::sema::Block<'a>) {
    for stmt in block.statements {
        compile_statement(ctx, stmt);
    }
}

fn compile_statement<'a>(ctx: &mut MirBuilderContext<'a>, stmt: crate::sema::Statement<'a>) {
    match stmt {
        crate::sema::Statement::Let { name, value, .. } => {
            let val_op = compile_expr(ctx, value.clone());
            let var_id = ctx.declare_var(name, value.ty);
            ctx.push_statement(Statement::Assign(var_id, Rvalue::Use(val_op)));
        }
        crate::sema::Statement::Assign { name, is_deref, value } => {
            let val_op = compile_expr(ctx, value);
            if is_deref {
                if let Some(var_id) = ctx.var_map.get(name).copied() {
                    ctx.push_statement(Statement::Store(var_id, val_op));
                } else {
                    ctx.push_statement(Statement::StoreVar(name, val_op));
                }
            } else {
                if let Some(var_id) = ctx.var_map.get(name).copied() {
                    ctx.push_statement(Statement::Assign(var_id, Rvalue::Use(val_op)));
                } else {
                    ctx.push_statement(Statement::AssignVar(name, val_op));
                }
            }
        }
        crate::sema::Statement::Expr(expr) => {
            compile_expr(ctx, expr);
        }
        crate::sema::Statement::Return(opt_expr) => {
            let term_op = opt_expr.map(|e| compile_expr(ctx, e));
            ctx.terminate(Terminator::Return(term_op));
        }
        crate::sema::Statement::If { cond, then_block, else_block } => {
            let cond_op = compile_expr(ctx, cond);
            let then_lbl = ctx.new_block();
            let else_lbl = ctx.new_block();
            let merge_lbl = ctx.new_block();

            ctx.terminate(Terminator::CondBranch {
                cond: cond_op,
                then_block: then_lbl,
                else_block: else_lbl,
            });

            // Compile then arm
            ctx.start_block(then_lbl);
            compile_block(ctx, then_block);
            if ctx.current_block.is_some() {
                ctx.terminate(Terminator::Goto(merge_lbl));
            }

            // Compile else arm
            ctx.start_block(else_lbl);
            if let Some(eb) = else_block {
                compile_block(ctx, eb);
            }
            if ctx.current_block.is_some() {
                ctx.terminate(Terminator::Goto(merge_lbl));
            }

            // Start merge block
            ctx.start_block(merge_lbl);
        }
        crate::sema::Statement::While { cond, body } => {
            let cond_lbl = ctx.new_block();
            let body_lbl = ctx.new_block();
            let end_lbl = ctx.new_block();

            ctx.terminate(Terminator::Goto(cond_lbl));

            // Compile condition block
            ctx.start_block(cond_lbl);
            let cond_op = compile_expr(ctx, cond);
            ctx.terminate(Terminator::CondBranch {
                cond: cond_op,
                then_block: body_lbl,
                else_block: end_lbl,
            });

            // Compile loop body
            ctx.start_block(body_lbl);
            ctx.loop_stack.push((cond_lbl, end_lbl));
            compile_block(ctx, body);
            ctx.loop_stack.pop();
            if ctx.current_block.is_some() {
                ctx.terminate(Terminator::Goto(cond_lbl));
            }

            // Start end block
            ctx.start_block(end_lbl);
        }
        crate::sema::Statement::Break => {
            if let Some((_, end_lbl)) = ctx.loop_stack.last().copied() {
                ctx.terminate(Terminator::Goto(end_lbl));
            }
        }
        crate::sema::Statement::Continue => {
            if let Some((cond_lbl, _)) = ctx.loop_stack.last().copied() {
                ctx.terminate(Terminator::Goto(cond_lbl));
            }
        }
    }
}

fn compile_expr<'a>(ctx: &mut MirBuilderContext<'a>, expr: crate::sema::ast::TypedExpr<'a>) -> Operand<'a> {
    match expr.kind {
        crate::sema::ast::ExprKind::Ident(name) => {
            if let Some(var_id) = ctx.var_map.get(name) {
                Operand::Var(*var_id)
            } else {
                Operand::Ident(name)
            }
        }
        crate::sema::ast::ExprKind::Int(val) => Operand::Int(val),
        crate::sema::ast::ExprKind::Float(val) => Operand::Float(val),
        crate::sema::ast::ExprKind::Bool(val) => Operand::Bool(val),
        crate::sema::ast::ExprKind::Binary { op, lhs, rhs } => {
            let lhs_op = compile_expr(ctx, *lhs);
            let rhs_op = compile_expr(ctx, *rhs);
            let temp_var = ctx.declare_temp(expr.ty);
            ctx.push_statement(Statement::Assign(
                temp_var,
                Rvalue::Binary(op, lhs_op, rhs_op),
            ));
            Operand::Var(temp_var)
        }
        crate::sema::ast::ExprKind::Unary { op, expr: sub_expr } => {
            let sub_op = compile_expr(ctx, *sub_expr);
            let temp_var = ctx.declare_temp(expr.ty);
            ctx.push_statement(Statement::Assign(
                temp_var,
                Rvalue::Unary(op, sub_op),
            ));
            Operand::Var(temp_var)
        }
         crate::sema::ast::ExprKind::Call { name, args } => {
            let mut arg_ops = Vec::new();
            for arg in args {
                arg_ops.push(compile_expr(ctx, arg));
            }
            if expr.ty == Type::Void {
                ctx.push_statement(Statement::Call(name, arg_ops));
                Operand::Int(0)
            } else {
                let temp_var = ctx.declare_temp(expr.ty);
                ctx.push_statement(Statement::Assign(
                    temp_var,
                    Rvalue::Call(name, arg_ops),
                ));
                Operand::Var(temp_var)
            }
        }
        crate::sema::ast::ExprKind::As { expr: sub_expr, ty } => {
            let sub_op = compile_expr(ctx, *sub_expr);
            let temp_var = ctx.declare_temp(expr.ty);
            ctx.push_statement(Statement::Assign(
                temp_var,
                Rvalue::As(sub_op, ty),
            ));
            Operand::Var(temp_var)
        }
        crate::sema::ast::ExprKind::Borrow { is_mut, expr: sub_expr } => {
            let temp_var = ctx.declare_temp(expr.ty.clone());
            match &sub_expr.kind {
                crate::sema::ast::ExprKind::Ident(name) => {
                    if let Some(var_id) = ctx.var_map.get(name).copied() {
                        ctx.push_statement(Statement::Assign(temp_var, Rvalue::Ref(is_mut, var_id)));
                    } else {
                        ctx.push_statement(Statement::Assign(temp_var, Rvalue::RefVar(is_mut, name)));
                    }
                }
                _ => panic!("Borrow target must be an identifier"),
            }
            Operand::Var(temp_var)
        }
        crate::sema::ast::ExprKind::Deref(sub_expr) => {
            let sub_op = compile_expr(ctx, *sub_expr);
            let temp_var = ctx.declare_temp(expr.ty.clone());
            ctx.push_statement(Statement::Assign(temp_var, Rvalue::Deref(sub_op)));
            Operand::Var(temp_var)
        }
    }
}
