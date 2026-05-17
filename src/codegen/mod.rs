use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::{PointerValue, BasicValueEnum, BasicValue};
use inkwell::types::{BasicTypeEnum, BasicType};
use inkwell::IntPredicate;
use inkwell::FloatPredicate;
use std::collections::HashMap;
use crate::mir;
use crate::sema::ast::Type;
use crate::parser::{BinOp, UnOp};

pub struct Codegen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    struct_types: std::cell::RefCell<HashMap<String, inkwell::types::StructType<'ctx>>>,
}

impl<'ctx> Codegen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self {
            context,
            module,
            builder,
            struct_types: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn compile_program(self, program: mir::Program<'ctx>) -> Module<'ctx> {
        // Pre-create all struct types as opaque struct types first
        for s in &program.structs {
            let struct_ty = self.context.opaque_struct_type(s.name);
            self.struct_types.borrow_mut().insert(s.name.to_string(), struct_ty);
        }

        // Set the body for all struct types
        for s in &program.structs {
            let struct_ty = *self.struct_types.borrow().get(s.name).unwrap();
            let field_types: Vec<BasicTypeEnum<'ctx>> = s.fields.iter()
                .map(|p| self.get_llvm_type(p.ty.clone()))
                .collect();
            struct_ty.set_body(&field_types, false);
        }

        // Pre-register all function signatures first to support mutual/forward calls
        for f in &program.functions {
            let param_types: Vec<BasicTypeEnum<'ctx>> = f.params.iter()
                .map(|(_, ty)| self.get_llvm_type(ty.clone()))
                .collect();
            
            let inkwell_param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = param_types.iter()
                .map(|t| (*t).into())
                .collect();

            let fn_type = if f.ret_type == Type::Void {
                self.context.void_type().fn_type(&inkwell_param_types, false)
            } else {
                self.get_llvm_type(f.ret_type.clone()).fn_type(&inkwell_param_types, false)
            };

            self.module.add_function(f.name, fn_type, None);
        }

        for f in program.functions {
            self.compile_function(f);
        }
        self.module
    }

    fn get_llvm_type(&self, ty: Type) -> BasicTypeEnum<'ctx> {
        match ty {
            Type::I64 => self.context.i64_type().as_basic_type_enum(),
            Type::F64 => self.context.f64_type().as_basic_type_enum(),
            Type::Bool => self.context.bool_type().as_basic_type_enum(),
            Type::Void => panic!("Void type cannot be converted to BasicTypeEnum"),
            Type::Ref { ty, .. } => {
                let inner = self.get_llvm_type(*ty);
                inner.ptr_type(inkwell::AddressSpace::default()).as_basic_type_enum()
            }
            Type::Struct(name) => {
                let struct_types = self.struct_types.borrow();
                let struct_ty = struct_types.get(&name)
                    .unwrap_or_else(|| panic!("Struct '{}' not registered in LLVM", name));
                struct_ty.as_basic_type_enum()
            }
        }
    }

    fn compile_function(&self, f: mir::Function<'ctx>) {
        let fn_val = self.module.get_function(f.name).unwrap();

        // Prepend an entry block for allocations to prevent PHI node requirements
        let alloc_bb = self.context.append_basic_block(fn_val, "entry_allocas");
        self.builder.position_at_end(alloc_bb);

        let mut param_ptrs = HashMap::new();
        let mut var_ptrs = HashMap::new();

        // Allocate and store function parameters
        for (i, (name, ty)) in f.params.iter().enumerate() {
            let llvm_ty = self.get_llvm_type(ty.clone());
            let ptr = self.builder.build_alloca(llvm_ty, name).unwrap();
            let val = fn_val.get_nth_param(i as u32).unwrap();
            self.builder.build_store(ptr, val).unwrap();
            param_ptrs.insert(*name, ptr);
        }

        // Allocate local variables and temporaries
        for (i, ty) in f.vars.iter().enumerate() {
            let llvm_ty = self.get_llvm_type(ty.clone());
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
                        let dest_ty = f.vars[var_id.0].clone();
                        let val = self.compile_rvalue(&rvalue, dest_ty, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(ptr, val).unwrap();
                    }
                    mir::Statement::AssignVar(name, operand) => {
                        let ptr = *param_ptrs.get(name).unwrap();
                        let val = self.compile_operand(&operand, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(ptr, val).unwrap();
                    }
                    mir::Statement::Store(var_id, operand) => {
                        let stack_ptr = *var_ptrs.get(&var_id).unwrap();
                        let ptr_val_ty = self.get_llvm_type(f.vars[var_id.0].clone());
                        let ptr_val = self.builder.build_load(ptr_val_ty, stack_ptr, "load_ptr_val").unwrap().into_pointer_value();
                        let val = self.compile_operand(&operand, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(ptr_val, val).unwrap();
                    }
                    mir::Statement::StoreVar(name, operand) => {
                        let stack_ptr = *param_ptrs.get(name).unwrap();
                        let param_ty = f.params.iter().find(|(n, _)| *n == name).map(|(_, t)| t.clone()).unwrap();
                        let ptr_val_ty = self.get_llvm_type(param_ty);
                        let ptr_val = self.builder.build_load(ptr_val_ty, stack_ptr, "load_param_ptr_val").unwrap().into_pointer_value();
                        let val = self.compile_operand(&operand, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(ptr_val, val).unwrap();
                    }
                    mir::Statement::Call(name, args) => {
                        let fn_val = self.module.get_function(name)
                            .unwrap_or_else(|| panic!("Function {} not found in LLVM module", name));
                        let mut compiled_args = Vec::new();
                        for arg in &args {
                            let val = self.compile_operand(arg, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                            compiled_args.push(val.into());
                        }
                        self.builder.build_call(fn_val, &compiled_args, "call_void_tmp").unwrap();
                    }
                    mir::Statement::AssignField(var_id, field_index, operand) => {
                        let var_ptr = *var_ptrs.get(&var_id).unwrap();
                        let struct_ty = self.get_llvm_type(f.vars[var_id.0].clone()).into_struct_type();
                        let field_ptr = self.builder.build_struct_gep(struct_ty, var_ptr, field_index as u32, "field_ptr").unwrap();
                        let val = self.compile_operand(&operand, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(field_ptr, val).unwrap();
                    }
                    mir::Statement::AssignFieldVar(name, field_index, operand) => {
                        let var_ptr = *param_ptrs.get(name).unwrap();
                        let param_ty = f.params.iter().find(|(n, _)| *n == name).map(|(_, t)| t.clone()).unwrap();
                        let struct_ty = self.get_llvm_type(param_ty).into_struct_type();
                        let field_ptr = self.builder.build_struct_gep(struct_ty, var_ptr, field_index as u32, "field_ptr").unwrap();
                        let val = self.compile_operand(&operand, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(field_ptr, val).unwrap();
                    }
                    mir::Statement::StoreField(var_id, field_index, operand) => {
                        let stack_ptr = *var_ptrs.get(&var_id).unwrap();
                        let ptr_val_ty = self.get_llvm_type(f.vars[var_id.0].clone());
                        let struct_ptr = self.builder.build_load(ptr_val_ty, stack_ptr, "load_struct_ptr").unwrap().into_pointer_value();
                        
                        let ref_ty = match f.vars[var_id.0].clone() {
                            Type::Ref { ty, .. } => *ty,
                            _ => unreachable!(),
                        };
                        let struct_ty = self.get_llvm_type(ref_ty).into_struct_type();
                        let field_ptr = self.builder.build_struct_gep(struct_ty, struct_ptr, field_index as u32, "field_ptr").unwrap();
                        let val = self.compile_operand(&operand, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(field_ptr, val).unwrap();
                    }
                    mir::Statement::StoreFieldVar(name, field_index, operand) => {
                        let stack_ptr = *param_ptrs.get(name).unwrap();
                        let param_ty = f.params.iter().find(|(n, _)| *n == name).map(|(_, t)| t.clone()).unwrap();
                        let ptr_val_ty = self.get_llvm_type(param_ty.clone());
                        let struct_ptr = self.builder.build_load(ptr_val_ty, stack_ptr, "load_struct_ptr_var").unwrap().into_pointer_value();
                        
                        let ref_ty = match param_ty {
                            Type::Ref { ty, .. } => *ty,
                            _ => unreachable!(),
                        };
                        let struct_ty = self.get_llvm_type(ref_ty).into_struct_type();
                        let field_ptr = self.builder.build_struct_gep(struct_ty, struct_ptr, field_index as u32, "field_ptr").unwrap();
                        let val = self.compile_operand(&operand, &var_ptrs, &param_ptrs, &f.vars, &f.params);
                        self.builder.build_store(field_ptr, val).unwrap();
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
            mir::Operand::Float(val) => {
                self.context.f64_type().const_float(*val).into()
            }
            mir::Operand::Bool(val) => {
                self.context.bool_type().const_int(if *val { 1 } else { 0 }, false).into()
            }
            mir::Operand::Var(vid) => {
                let ptr = *var_ptrs.get(vid).unwrap();
                let ty = self.get_llvm_type(vars[vid.0].clone());
                self.builder.build_load(ty, ptr, &format!("load_var_{}", vid.0)).unwrap()
            }
            mir::Operand::Ident(name) => {
                let ptr = *param_ptrs.get(name).unwrap();
                let param_ty = params.iter().find(|(n, _)| *n == *name).map(|(_, t)| t.clone()).unwrap();
                let ty = self.get_llvm_type(param_ty);
                self.builder.build_load(ty, ptr, &format!("load_param_{}", name)).unwrap()
            }
        }
    }

    fn compile_rvalue(
        &self,
        rvalue: &mir::Rvalue<'ctx>,
        dest_ty: Type,
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
                        if val.is_float_value() {
                            self.builder.build_float_neg(val.into_float_value(), "fneg_val").unwrap().into()
                        } else {
                            let int_val = val.into_int_value();
                            self.builder.build_int_neg(int_val, "neg_val").unwrap().into()
                        }
                    }
                }
            }
            mir::Rvalue::Binary(op, lhs_op, rhs_op) => {
                let lhs_val = self.compile_operand(lhs_op, var_ptrs, param_ptrs, vars, params);
                let rhs_val = self.compile_operand(rhs_op, var_ptrs, param_ptrs, vars, params);
                let is_float = lhs_val.is_float_value();

                if is_float {
                    let l = lhs_val.into_float_value();
                    let r = rhs_val.into_float_value();
                    match op {
                        BinOp::Add => self.builder.build_float_add(l, r, "fadd_tmp").unwrap().into(),
                        BinOp::Sub => self.builder.build_float_sub(l, r, "fsub_tmp").unwrap().into(),
                        BinOp::Mul => self.builder.build_float_mul(l, r, "fmul_tmp").unwrap().into(),
                        BinOp::Div => self.builder.build_float_div(l, r, "fdiv_tmp").unwrap().into(),
                        
                        BinOp::Eq => self.builder.build_float_compare(FloatPredicate::OEQ, l, r, "feq_tmp").unwrap().into(),
                        BinOp::Ne => self.builder.build_float_compare(FloatPredicate::UNE, l, r, "fne_tmp").unwrap().into(),
                        BinOp::Lt => self.builder.build_float_compare(FloatPredicate::OLT, l, r, "flt_tmp").unwrap().into(),
                        BinOp::Le => self.builder.build_float_compare(FloatPredicate::OLE, l, r, "fle_tmp").unwrap().into(),
                        BinOp::Gt => self.builder.build_float_compare(FloatPredicate::OGT, l, r, "fgt_tmp").unwrap().into(),
                        BinOp::Ge => self.builder.build_float_compare(FloatPredicate::OGE, l, r, "fge_tmp").unwrap().into(),
                        
                        BinOp::And | BinOp::Or => panic!("Logical operators not supported on float values"),
                    }
                } else {
                    let l = lhs_val.into_int_value();
                    let r = rhs_val.into_int_value();
                    match op {
                        BinOp::Add => self.builder.build_int_add(l, r, "add_tmp").unwrap().into(),
                        BinOp::Sub => self.builder.build_int_sub(l, r, "sub_tmp").unwrap().into(),
                        BinOp::Mul => self.builder.build_int_mul(l, r, "mul_tmp").unwrap().into(),
                        BinOp::Div => self.builder.build_int_signed_div(l, r, "div_tmp").unwrap().into(),
                        
                        BinOp::Eq => self.builder.build_int_compare(IntPredicate::EQ, l, r, "eq_tmp").unwrap().into(),
                        BinOp::Ne => self.builder.build_int_compare(IntPredicate::NE, l, r, "ne_tmp").unwrap().into(),
                        BinOp::Lt => self.builder.build_int_compare(IntPredicate::SLT, l, r, "lt_tmp").unwrap().into(),
                        BinOp::Le => self.builder.build_int_compare(IntPredicate::SLE, l, r, "le_tmp").unwrap().into(),
                        BinOp::Gt => self.builder.build_int_compare(IntPredicate::SGT, l, r, "gt_tmp").unwrap().into(),
                        BinOp::Ge => self.builder.build_int_compare(IntPredicate::SGE, l, r, "fge_tmp").unwrap().into(),
                        
                        BinOp::And => self.builder.build_and(l, r, "and_tmp").unwrap().into(),
                        BinOp::Or => self.builder.build_or(l, r, "or_tmp").unwrap().into(),
                    }
                }
            }
            mir::Rvalue::Call(name, args) => {
                let fn_val = self.module.get_function(name)
                    .unwrap_or_else(|| panic!("Function {} not found in LLVM module", name));
                let mut compiled_args = Vec::new();
                for arg in args {
                    let val = self.compile_operand(arg, var_ptrs, param_ptrs, vars, params);
                    compiled_args.push(val.into());
                }
                let call_val = self.builder.build_call(fn_val, &compiled_args, "call_tmp").unwrap();
                match call_val.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(val) => val,
                    inkwell::values::ValueKind::Instruction(_) => panic!("Expected function call to return a value"),
                }
            }
            mir::Rvalue::As(op, dest_ty) => {
                let val = self.compile_operand(op, var_ptrs, param_ptrs, vars, params);
                let src_ty = match op {
                    mir::Operand::Var(vid) => vars[vid.0].clone(),
                    mir::Operand::Int(_) => Type::I64,
                    mir::Operand::Float(_) => Type::F64,
                    mir::Operand::Bool(_) => Type::Bool,
                    mir::Operand::Ident(name) => params.iter().find(|(n, _)| *n == *name).map(|(_, t)| t.clone()).unwrap(),
                };
                
                match (src_ty, dest_ty) {
                    (Type::I64, Type::F64) => {
                        self.builder.build_signed_int_to_float(val.into_int_value(), self.context.f64_type(), "cast_i64_f64").unwrap().into()
                    }
                    (Type::F64, Type::I64) => {
                        self.builder.build_float_to_signed_int(val.into_float_value(), self.context.i64_type(), "cast_f64_i64").unwrap().into()
                    }
                    _ => val,
                }
            }
            mir::Rvalue::Ref(_is_mut, var_id) => {
                let ptr = *var_ptrs.get(var_id).unwrap();
                ptr.into()
            }
            mir::Rvalue::RefVar(_is_mut, name) => {
                let ptr = *param_ptrs.get(name).unwrap();
                ptr.into()
            }
            mir::Rvalue::Deref(op) => {
                let ptr_val = self.compile_operand(op, var_ptrs, param_ptrs, vars, params).into_pointer_value();
                let llvm_dest_ty = self.get_llvm_type(dest_ty);
                self.builder.build_load(llvm_dest_ty, ptr_val, "deref_val").unwrap()
            }
            mir::Rvalue::StructLiteral(ops) => {
                let struct_ty = self.get_llvm_type(dest_ty.clone()).into_struct_type();
                let temp_alloca = self.builder.build_alloca(struct_ty, "struct_literal_alloca").unwrap();
                for (i, op) in ops.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(struct_ty, temp_alloca, i as u32, &format!("field_{}", i)).unwrap();
                    let val = self.compile_operand(op, var_ptrs, param_ptrs, vars, params);
                    self.builder.build_store(field_ptr, val).unwrap();
                }
                self.builder.build_load(struct_ty, temp_alloca, "loaded_struct").unwrap()
            }
            mir::Rvalue::FieldAccess(op, field_index) => {
                let (struct_ptr, struct_ty) = match op {
                    mir::Operand::Var(vid) => {
                        let ptr = *var_ptrs.get(vid).unwrap();
                        match vars[vid.0].clone() {
                            Type::Ref { ty, .. } => {
                                let ptr_val_ty = self.get_llvm_type(vars[vid.0].clone());
                                let struct_ptr = self.builder.build_load(ptr_val_ty, ptr, "load_struct_ptr").unwrap().into_pointer_value();
                                let struct_ty = self.get_llvm_type(*ty.clone()).into_struct_type();
                                (struct_ptr, struct_ty)
                            }
                            _ => {
                                let struct_ty = self.get_llvm_type(vars[vid.0].clone()).into_struct_type();
                                (ptr, struct_ty)
                            }
                        }
                    }
                    mir::Operand::Ident(name) => {
                        let ptr = *param_ptrs.get(name).unwrap();
                        let param_ty = params.iter().find(|(n, _)| *n == *name).map(|(_, t)| t.clone()).unwrap();
                        match param_ty {
                            Type::Ref { ref ty, .. } => {
                                let ptr_val_ty = self.get_llvm_type(param_ty.clone());
                                let struct_ptr = self.builder.build_load(ptr_val_ty, ptr, "load_struct_ptr").unwrap().into_pointer_value();
                                let struct_ty = self.get_llvm_type(*ty.clone()).into_struct_type();
                                (struct_ptr, struct_ty)
                            }
                            _ => {
                                let struct_ty = self.get_llvm_type(param_ty).into_struct_type();
                                (ptr, struct_ty)
                            }
                        }
                    }
                    _ => unreachable!(),
                };
                let field_ptr = self.builder.build_struct_gep(struct_ty, struct_ptr, *field_index as u32, "field_ptr").unwrap();
                let field_llvm_ty = self.get_llvm_type(dest_ty.clone());
                self.builder.build_load(field_llvm_ty, field_ptr, "loaded_field").unwrap()
            }
            mir::Rvalue::RefField(_is_mut, vid, field_index) => {
                let (struct_ptr, struct_ty) = match vars[vid.0].clone() {
                    Type::Ref { ty, .. } => {
                        let ptr = *var_ptrs.get(vid).unwrap();
                        let ptr_val_ty = self.get_llvm_type(vars[vid.0].clone());
                        let struct_ptr = self.builder.build_load(ptr_val_ty, ptr, "load_struct_ptr").unwrap().into_pointer_value();
                        let struct_ty = self.get_llvm_type(*ty.clone()).into_struct_type();
                        (struct_ptr, struct_ty)
                    }
                    _ => {
                        let ptr = *var_ptrs.get(vid).unwrap();
                        let struct_ty = self.get_llvm_type(vars[vid.0].clone()).into_struct_type();
                        (ptr, struct_ty)
                    }
                };
                let field_ptr = self.builder.build_struct_gep(struct_ty, struct_ptr, *field_index as u32, "field_ptr").unwrap();
                field_ptr.as_basic_value_enum()
            }
            mir::Rvalue::RefFieldVar(_is_mut, name, field_index) => {
                let ptr = *param_ptrs.get(name).unwrap();
                let param_ty = params.iter().find(|(n, _)| *n == *name).map(|(_, t)| t.clone()).unwrap();
                let (struct_ptr, struct_ty) = match param_ty {
                    Type::Ref { ref ty, .. } => {
                        let ptr_val_ty = self.get_llvm_type(param_ty.clone());
                        let struct_ptr = self.builder.build_load(ptr_val_ty, ptr, "load_struct_ptr").unwrap().into_pointer_value();
                        let struct_ty = self.get_llvm_type(*ty.clone()).into_struct_type();
                        (struct_ptr, struct_ty)
                    }
                    _ => {
                        let struct_ty = self.get_llvm_type(param_ty).into_struct_type();
                        (ptr, struct_ty)
                    }
                };
                let field_ptr = self.builder.build_struct_gep(struct_ty, struct_ptr, *field_index as u32, "field_ptr").unwrap();
                field_ptr.as_basic_value_enum()
            }
        }
    }
}
