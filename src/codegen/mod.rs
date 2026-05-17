use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::{PointerValue, BasicValueEnum};
use inkwell::types::{BasicTypeEnum, BasicType};
use inkwell::IntPredicate;
use std::collections::HashMap;
use crate::mir;
use crate::sema::ast::Type;
use crate::parser::{BinOp, UnOp};

pub struct Codegen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
}

impl<'ctx> Codegen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self { context, module, builder }
    }

    pub fn compile_program(self, program: mir::Program<'ctx>) -> Module<'ctx> {
        for f in program.functions {
            self.compile_function(f);
        }
        self.module
    }

    fn get_llvm_type(&self, ty: Type) -> BasicTypeEnum<'ctx> {
        match ty {
            Type::I64 => self.context.i64_type().as_basic_type_enum(),
            Type::Bool => self.context.bool_type().as_basic_type_enum(),
            Type::Void => panic!("Void type cannot be converted to BasicTypeEnum"),
        }
    }

    fn compile_function(&self, f: mir::Function<'ctx>) {
        let param_types: Vec<BasicTypeEnum<'ctx>> = f.params.iter()
            .map(|(_, ty)| self.get_llvm_type(*ty))
            .collect();
        
        let inkwell_param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = param_types.iter()
            .map(|t| (*t).into())
            .collect();

        let fn_type = if f.ret_type == Type::Void {
            self.context.void_type().fn_type(&inkwell_param_types, false)
        } else {
            self.get_llvm_type(f.ret_type).fn_type(&inkwell_param_types, false)
        };

        let fn_val = self.module.add_function(f.name, fn_type, None);

        // Prepend an entry block for allocations to prevent PHI node requirements
        let alloc_bb = self.context.append_basic_block(fn_val, "entry_allocas");
        self.builder.position_at_end(alloc_bb);

        let mut param_ptrs = HashMap::new();
        let mut var_ptrs = HashMap::new();

        // Allocate and store function parameters
        for (i, (name, ty)) in f.params.iter().enumerate() {
            let llvm_ty = self.get_llvm_type(*ty);
            let ptr = self.builder.build_alloca(llvm_ty, name).unwrap();
            let val = fn_val.get_nth_param(i as u32).unwrap();
            self.builder.build_store(ptr, val).unwrap();
            param_ptrs.insert(*name, ptr);
        }

        // Allocate local variables and temporaries
        for (i, ty) in f.vars.iter().enumerate() {
            let llvm_ty = self.get_llvm_type(*ty);
            let ptr = self.builder.build_alloca(llvm_ty, &format!("var_{}", i)).unwrap();
            var_ptrs.insert(mir::VarId(i), ptr);
        }

        // Build mapping of MIR blocks to LLVM Blocks
        let mut llvm_blocks = HashMap::new();
        for bb in &f.basic_blocks {
            let llvm_block = self.context.append_basic_block(fn_val, &format!("bb_{}", bb.id.0));
            llvm_blocks.insert(bb.id, llvm_block);
        }

        // Branch from allocations to the first MIR basic block
        if let Some(first_bb) = llvm_blocks.get(&mir::BasicBlockId(0)) {
            self.builder.build_unconditional_branch(*first_bb).unwrap();
        }

        // Compile MIR Basic Blocks
        for bb in f.basic_blocks {
            let llvm_bb = *llvm_blocks.get(&bb.id).unwrap();
            self.builder.position_at_end(llvm_bb);

            for stmt in bb.statements {
                match stmt {
                    mir::Statement::Assign(var_id, rvalue) => {
                        let ptr = *var_ptrs.get(&var_id).unwrap();
                        let val = self.compile_rvalue(&rvalue, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(ptr, val).unwrap();
                    }
                    mir::Statement::AssignVar(name, operand) => {
                        let ptr = *param_ptrs.get(name).unwrap();
                        let val = self.compile_operand(&operand, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(ptr, val).unwrap();
                    }
                }
            }

            match bb.terminator {
                mir::Terminator::Return(opt_op) => {
                    if let Some(op) = opt_op {
                        let val = self.compile_operand(&op, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_return(Some(&val)).unwrap();
                    } else {
                        self.builder.build_return(None).unwrap();
                    }
                }
                mir::Terminator::Goto(target) => {
                    let target_bb = *llvm_blocks.get(&target).unwrap();
                    self.builder.build_unconditional_branch(target_bb).unwrap();
                }
                mir::Terminator::CondBranch { cond, then_block, else_block } => {
                    let cond_val = self.compile_operand(&cond, &var_ptrs, &param_ptrs, &f.vars, &f.params).into_int_value();
                    let then_bb = *llvm_blocks.get(&then_block).unwrap();
                    let else_bb = *llvm_blocks.get(&else_block).unwrap();
                    self.builder.build_conditional_branch(cond_val, then_bb, else_bb).unwrap();
                }
                mir::Terminator::Unreachable => {
                    self.builder.build_unreachable().unwrap();
                }
            }
        }
    }

    fn compile_operand(
        &self,
        op: &mir::Operand<'ctx>,
        var_ptrs: &HashMap<mir::VarId, PointerValue<'ctx>>,
        param_ptrs: &HashMap<&str, PointerValue<'ctx>>,
        vars: &[Type],
        params: &[(&str, Type)],
    ) -> BasicValueEnum<'ctx> {
        match op {
            mir::Operand::Int(val) => {
                self.context.i64_type().const_int(*val as u64, false).into()
            }
            mir::Operand::Bool(val) => {
                self.context.bool_type().const_int(if *val { 1 } else { 0 }, false).into()
            }
            mir::Operand::Var(vid) => {
                let ptr = *var_ptrs.get(vid).unwrap();
                let ty = self.get_llvm_type(vars[vid.0]);
                self.builder.build_load(ty, ptr, &format!("load_var_{}", vid.0)).unwrap()
            }
            mir::Operand::Ident(name) => {
                let ptr = *param_ptrs.get(name).unwrap();
                let param_ty = params.iter().find(|(n, _)| *n == *name).map(|(_, t)| *t).unwrap();
                let ty = self.get_llvm_type(param_ty);
                self.builder.build_load(ty, ptr, &format!("load_param_{}", name)).unwrap()
            }
        }
    }

    fn compile_rvalue(
        &self,
        rvalue: &mir::Rvalue<'ctx>,
        var_ptrs: &HashMap<mir::VarId, PointerValue<'ctx>>,
        param_ptrs: &HashMap<&str, PointerValue<'ctx>>,
        vars: &[Type],
        params: &[(&str, Type)],
    ) -> BasicValueEnum<'ctx> {
        match rvalue {
            mir::Rvalue::Use(op) => {
                self.compile_operand(op, var_ptrs, param_ptrs, vars, params)
            }
            mir::Rvalue::Unary(op, sub_op) => {
                let val = self.compile_operand(sub_op, var_ptrs, param_ptrs, vars, params);
                match op {
                    UnOp::Not => {
                        let int_val = val.into_int_value();
                        self.builder.build_not(int_val, "not_val").unwrap().into()
                    }
                    UnOp::Neg => {
                        let int_val = val.into_int_value();
                        self.builder.build_int_neg(int_val, "neg_val").unwrap().into()
                    }
                }
            }
            mir::Rvalue::Binary(op, lhs_op, rhs_op) => {
                let lhs_val = self.compile_operand(lhs_op, var_ptrs, param_ptrs, vars, params);
                let rhs_val = self.compile_operand(rhs_op, var_ptrs, param_ptrs, vars, params);
                match op {
                    BinOp::Add => self.builder.build_int_add(lhs_val.into_int_value(), rhs_val.into_int_value(), "add_tmp").unwrap().into(),
                    BinOp::Sub => self.builder.build_int_sub(lhs_val.into_int_value(), rhs_val.into_int_value(), "sub_tmp").unwrap().into(),
                    BinOp::Mul => self.builder.build_int_mul(lhs_val.into_int_value(), rhs_val.into_int_value(), "mul_tmp").unwrap().into(),
                    BinOp::Div => self.builder.build_int_signed_div(lhs_val.into_int_value(), rhs_val.into_int_value(), "div_tmp").unwrap().into(),
                    
                    BinOp::Eq => self.builder.build_int_compare(IntPredicate::EQ, lhs_val.into_int_value(), rhs_val.into_int_value(), "eq_tmp").unwrap().into(),
                    BinOp::Ne => self.builder.build_int_compare(IntPredicate::NE, lhs_val.into_int_value(), rhs_val.into_int_value(), "ne_tmp").unwrap().into(),
                    BinOp::Lt => self.builder.build_int_compare(IntPredicate::SLT, lhs_val.into_int_value(), rhs_val.into_int_value(), "lt_tmp").unwrap().into(),
                    BinOp::Le => self.builder.build_int_compare(IntPredicate::SLE, lhs_val.into_int_value(), rhs_val.into_int_value(), "le_tmp").unwrap().into(),
                    BinOp::Gt => self.builder.build_int_compare(IntPredicate::SGT, lhs_val.into_int_value(), rhs_val.into_int_value(), "gt_tmp").unwrap().into(),
                    BinOp::Ge => self.builder.build_int_compare(IntPredicate::SGE, lhs_val.into_int_value(), rhs_val.into_int_value(), "ge_tmp").unwrap().into(),
                    
                    BinOp::And => self.builder.build_and(lhs_val.into_int_value(), rhs_val.into_int_value(), "and_tmp").unwrap().into(),
                    BinOp::Or => self.builder.build_or(lhs_val.into_int_value(), rhs_val.into_int_value(), "or_tmp").unwrap().into(),
                }
            }
        }
    }
}
