use crate::mir::ast::{
    BasicBlock, BasicBlockId, Function, Operand, Program, Rvalue, Statement, Terminator, VarId,
};
use crate::sema::ast::Type;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place<'a> {
    Local(VarId),
    Param(&'a str),
}

impl<'a> Place<'a> {
    fn name(&self, _func: &Function<'a>) -> String {
        match self {
            Place::Local(var_id) => {
                format!("_t{}", var_id.0)
            }
            Place::Param(name) => name.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Loan<'a> {
    pub place: Place<'a>,
    pub is_mut: bool,
}

#[allow(dead_code)]
struct Liveness<'a> {
    live_in: HashMap<BasicBlockId, HashSet<Place<'a>>>,
    live_out: HashMap<BasicBlockId, HashSet<Place<'a>>>,
}

fn get_operand_place<'a>(op: &Operand<'a>) -> Option<Place<'a>> {
    match op {
        Operand::Var(vid) => Some(Place::Local(*vid)),
        Operand::Ident(name) => Some(Place::Param(name)),
        _ => None,
    }
}

fn add_rvalue_uses<'a>(rvalue: &Rvalue<'a>, uses: &mut Vec<Place<'a>>) {
    match rvalue {
        Rvalue::Use(op) => {
            if let Some(p) = get_operand_place(op) {
                uses.push(p);
            }
        }
        Rvalue::Binary(_, op1, op2) => {
            if let Some(p) = get_operand_place(op1) {
                uses.push(p);
            }
            if let Some(p) = get_operand_place(op2) {
                uses.push(p);
            }
        }
        Rvalue::Unary(_, op) => {
            if let Some(p) = get_operand_place(op) {
                uses.push(p);
            }
        }
        Rvalue::Call(_, args) => {
            for arg in args {
                if let Some(p) = get_operand_place(arg) {
                    uses.push(p);
                }
            }
        }
        Rvalue::As(op, _) => {
            if let Some(p) = get_operand_place(op) {
                uses.push(p);
            }
        }
        Rvalue::Ref(_, vid) => {
            uses.push(Place::Local(*vid));
        }
        Rvalue::RefVar(_, name) => {
            uses.push(Place::Param(name));
        }
        Rvalue::Deref(op) => {
            if let Some(p) = get_operand_place(op) {
                uses.push(p);
            }
        }
        Rvalue::StructLiteral(ops) => {
            for op in ops {
                if let Some(p) = get_operand_place(op) {
                    uses.push(p);
                }
            }
        }
        Rvalue::FieldAccess(op, _) => {
            if let Some(p) = get_operand_place(op) {
                uses.push(p);
            }
        }
        Rvalue::RefField(_, vid, _) => {
            uses.push(Place::Local(*vid));
        }
        Rvalue::RefFieldVar(_, name, _) => {
            uses.push(Place::Param(name));
        }
        Rvalue::New(sub_rvalue) => {
            add_rvalue_uses(sub_rvalue, uses);
        }
    }
}

fn get_stmt_defs_uses<'a>(stmt: &Statement<'a>) -> (Option<Place<'a>>, Vec<Place<'a>>) {
    let mut def = None;
    let mut uses = Vec::new();
    match stmt {
        Statement::Assign(var_id, rvalue) => {
            def = Some(Place::Local(*var_id));
            add_rvalue_uses(rvalue, &mut uses);
        }
        Statement::AssignVar(name, operand) => {
            def = Some(Place::Param(name));
            if let Some(p) = get_operand_place(operand) {
                uses.push(p);
            }
        }
        Statement::Store(var_id, operand) => {
            uses.push(Place::Local(*var_id));
            if let Some(p) = get_operand_place(operand) {
                uses.push(p);
            }
        }
        Statement::StoreVar(name, operand) => {
            uses.push(Place::Param(name));
            if let Some(p) = get_operand_place(operand) {
                uses.push(p);
            }
        }
        Statement::Call(_, args) => {
            for arg in args {
                if let Some(p) = get_operand_place(arg) {
                    uses.push(p);
                }
            }
        }
        Statement::AssignField(var_id, _, operand) => {
            def = Some(Place::Local(*var_id));
            if let Some(p) = get_operand_place(operand) {
                uses.push(p);
            }
        }
        Statement::AssignFieldVar(name, _, operand) => {
            def = Some(Place::Param(name));
            if let Some(p) = get_operand_place(operand) {
                uses.push(p);
            }
        }
        Statement::StoreField(var_id, _, operand) => {
            uses.push(Place::Local(*var_id));
            if let Some(p) = get_operand_place(operand) {
                uses.push(p);
            }
        }
        Statement::StoreFieldVar(name, _, operand) => {
            uses.push(Place::Param(name));
            if let Some(p) = get_operand_place(operand) {
                uses.push(p);
            }
        }
    }
    (def, uses)
}

fn get_term_uses<'a>(term: &Terminator<'a>) -> Vec<Place<'a>> {
    let mut uses = Vec::new();
    match term {
        Terminator::Return(Some(op)) => {
            if let Some(p) = get_operand_place(op) {
                uses.push(p);
            }
        }
        Terminator::CondBranch { cond, .. } => {
            if let Some(p) = get_operand_place(cond) {
                uses.push(p);
            }
        }
        _ => {}
    }
    uses
}

fn get_successors(term: &Terminator<'_>) -> Vec<BasicBlockId> {
    match term {
        Terminator::Goto(target) => vec![*target],
        Terminator::CondBranch { then_block, else_block, .. } => vec![*then_block, *else_block],
        _ => vec![],
    }
}

fn run_liveness<'a>(func: &Function<'a>) -> Liveness<'a> {
    let mut live_in: HashMap<BasicBlockId, HashSet<Place<'a>>> = HashMap::new();
    let mut live_out: HashMap<BasicBlockId, HashSet<Place<'a>>> = HashMap::new();

    for bb in &func.basic_blocks {
        live_in.insert(bb.id, HashSet::new());
        live_out.insert(bb.id, HashSet::new());
    }

    let mut changed = true;
    while changed {
        changed = false;
        for bb in &func.basic_blocks {
            let mut new_live_out = HashSet::new();
            for succ in get_successors(&bb.terminator) {
                if let Some(in_set) = live_in.get(&succ) {
                    new_live_out.extend(in_set.clone());
                }
            }

            let mut current = new_live_out.clone();
            
            for u in get_term_uses(&bb.terminator) {
                current.insert(u);
            }

            for stmt in bb.statements.iter().rev() {
                let (def, uses) = get_stmt_defs_uses(stmt);
                if let Some(d) = def {
                    current.remove(&d);
                }
                for u in uses {
                    current.insert(u);
                }
            }

            let old_live_in = live_in.get_mut(&bb.id).unwrap();
            if *old_live_in != current {
                *old_live_in = current;
                changed = true;
            }
            
            live_out.insert(bb.id, new_live_out);
        }
    }

    Liveness { live_in, live_out }
}

fn merge_borrow_states<'a>(
    states: &[&HashMap<Place<'a>, HashSet<Loan<'a>>>],
) -> HashMap<Place<'a>, HashSet<Loan<'a>>> {
    let mut merged = HashMap::new();
    for state in states {
        for (ref_place, loans) in *state {
            merged.entry(*ref_place)
                .or_insert_with(HashSet::new)
                .extend(loans.clone());
        }
    }
    merged
}

fn transfer_statement<'a>(
    state: &mut HashMap<Place<'a>, HashSet<Loan<'a>>>,
    stmt: &Statement<'a>,
) {
    match stmt {
        Statement::Assign(var_id, rvalue) => {
            let def_place = Place::Local(*var_id);
            state.remove(&def_place);
            match rvalue {
                Rvalue::Ref(is_mut, target_vid) => {
                    let loan = Loan {
                        place: Place::Local(*target_vid),
                        is_mut: *is_mut,
                    };
                    state.entry(def_place).or_default().insert(loan);
                }
                Rvalue::RefVar(is_mut, target_name) => {
                    let loan = Loan {
                        place: Place::Param(target_name),
                        is_mut: *is_mut,
                    };
                    state.entry(def_place).or_default().insert(loan);
                }
                Rvalue::RefField(is_mut, target_vid, _) => {
                    let loan = Loan {
                        place: Place::Local(*target_vid),
                        is_mut: *is_mut,
                    };
                    state.entry(def_place).or_default().insert(loan);
                }
                Rvalue::RefFieldVar(is_mut, target_name, _) => {
                    let loan = Loan {
                        place: Place::Param(target_name),
                        is_mut: *is_mut,
                    };
                    state.entry(def_place).or_default().insert(loan);
                }
                Rvalue::Use(Operand::Var(src_vid)) => {
                    let src_place = Place::Local(*src_vid);
                    if let Some(loans) = state.get(&src_place).cloned() {
                        state.insert(def_place, loans);
                    }
                }
                Rvalue::Use(Operand::Ident(src_name)) => {
                    let src_place = Place::Param(src_name);
                    if let Some(loans) = state.get(&src_place).cloned() {
                        state.insert(def_place, loans);
                    }
                }
                _ => {}
            }
        }
        Statement::AssignVar(name, operand) => {
            let def_place = Place::Param(name);
            state.remove(&def_place);
            match operand {
                Operand::Var(src_vid) => {
                    let src_place = Place::Local(*src_vid);
                    if let Some(loans) = state.get(&src_place).cloned() {
                        state.insert(def_place, loans);
                    }
                }
                Operand::Ident(src_name) => {
                    let src_place = Place::Param(src_name);
                    if let Some(loans) = state.get(&src_place).cloned() {
                        state.insert(def_place, loans);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn run_points_to<'a>(
    func: &Function<'a>,
) -> HashMap<BasicBlockId, HashMap<Place<'a>, HashSet<Loan<'a>>>> {
    let mut borrows_in = HashMap::new();
    let mut borrows_out = HashMap::new();

    for bb in &func.basic_blocks {
        borrows_in.insert(bb.id, HashMap::new());
        borrows_out.insert(bb.id, HashMap::new());
    }

    let mut predecessors: HashMap<BasicBlockId, Vec<BasicBlockId>> = HashMap::new();
    for bb in &func.basic_blocks {
        predecessors.entry(bb.id).or_default();
        for succ in get_successors(&bb.terminator) {
            predecessors.entry(succ).or_default().push(bb.id);
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for bb in &func.basic_blocks {
            let preds = predecessors.get(&bb.id).unwrap();
            let pred_states: Vec<&HashMap<Place<'a>, HashSet<Loan<'a>>>> = preds
                .iter()
                .map(|p_id| borrows_out.get(p_id).unwrap())
                .collect();
            
            let new_in = merge_borrow_states(&pred_states);
            
            if borrows_in.get(&bb.id).unwrap() != &new_in {
                borrows_in.insert(bb.id, new_in.clone());
                changed = true;
            }

            let mut current = new_in;
            for stmt in &bb.statements {
                transfer_statement(&mut current, stmt);
            }

            if borrows_out.get(&bb.id).unwrap() != &current {
                borrows_out.insert(bb.id, current);
                changed = true;
            }
        }
    }

    borrows_in
}

fn compute_live_points<'a>(
    bb: &BasicBlock<'a>,
    block_live_out: &HashSet<Place<'a>>,
) -> (Vec<HashSet<Place<'a>>>, HashSet<Place<'a>>) {
    let mut live_points = vec![HashSet::new(); bb.statements.len()];
    let mut current = block_live_out.clone();

    for u in get_term_uses(&bb.terminator) {
        current.insert(u);
    }
    let term_live = current.clone();

    for (i, stmt) in bb.statements.iter().enumerate().rev() {
        live_points[i] = current.clone();
        let (def, uses) = get_stmt_defs_uses(stmt);
        if let Some(d) = def {
            current.remove(&d);
        }
        for u in uses {
            current.insert(u);
        }
    }

    (live_points, term_live)
}

fn get_place_type<'a>(func: &Function<'a>, place: Place<'a>) -> Option<Type> {
    match place {
        Place::Local(var_id) => func.vars.get(var_id.0).cloned(),
        Place::Param(name) => {
            func.params.iter().find(|(p_name, _)| *p_name == name).map(|(_, ty)| ty.clone())
        }
    }
}

fn get_active_loans<'a>(
    borrows: &HashMap<Place<'a>, HashSet<Loan<'a>>>,
    live: &HashSet<Place<'a>>,
) -> HashSet<Loan<'a>> {
    let mut active = HashSet::new();
    for (ref_var, loans) in borrows {
        if live.contains(ref_var) {
            for loan in loans {
                active.insert(*loan);
            }
        }
    }
    active
}

fn check_borrow_conflict<'a>(
    func: &Function<'a>,
    x: Place<'a>,
    is_mut: bool,
    active_loans: &HashSet<Loan<'a>>,
) -> Result<(), String> {
    for loan in active_loans {
        if loan.place == x {
            if is_mut {
                return Err(format!(
                    "Cannot borrow '{}' as mutable because it is already borrowed",
                    x.name(func)
                ));
            } else if loan.is_mut {
                return Err(format!(
                    "Cannot borrow '{}' as immutable because it is already borrowed mutably",
                    x.name(func)
                ));
            }
        }
    }
    Ok(())
}

fn check_read_access<'a>(
    func: &Function<'a>,
    x: Place<'a>,
    active_loans: &HashSet<Loan<'a>>,
) -> Result<(), String> {
    for loan in active_loans {
        if loan.place == x && loan.is_mut {
            return Err(format!(
                "Cannot read variable '{}' because it is mutably borrowed",
                x.name(func)
            ));
        }
    }
    Ok(())
}

fn check_write_access<'a>(
    func: &Function<'a>,
    x: Place<'a>,
    active_loans: &HashSet<Loan<'a>>,
) -> Result<(), String> {
    for loan in active_loans {
        if loan.place == x {
            return Err(format!(
                "Cannot write to/mutate variable '{}' because it is borrowed",
                x.name(func)
            ));
        }
    }
    Ok(())
}

fn check_deref_write_access<'a>(
    func: &Function<'a>,
    v: Place<'a>,
    borrows: &HashMap<Place<'a>, HashSet<Loan<'a>>>,
    live: &HashSet<Place<'a>>,
) -> Result<(), String> {
    let ty = get_place_type(func, v)
        .ok_or_else(|| format!("Unknown variable '{}'", v.name(func)))?;
    match ty {
        Type::Ref { is_mut, .. } => {
            if !is_mut {
                return Err(format!(
                    "Cannot write through immutable reference '{}'",
                    v.name(func)
                ));
            }
        }
        _ => {
            return Err(format!(
                "Cannot dereference non-reference variable '{}'",
                v.name(func)
            ));
        }
    }

    if let Some(loans) = borrows.get(&v) {
        for loan in loans {
            let x = loan.place;
            for (other_ref, other_loans) in borrows {
                if other_ref != &v && live.contains(other_ref) {
                    for other_loan in other_loans {
                        if other_loan.place == x {
                            return Err(format!(
                                "Cannot write to '{}' through reference '{}' because it is also borrowed by '{}'",
                                x.name(func),
                                v.name(func),
                                other_ref.name(func)
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_stmt<'a>(
    func: &Function<'a>,
    stmt: &Statement<'a>,
    borrows: &HashMap<Place<'a>, HashSet<Loan<'a>>>,
    live: &HashSet<Place<'a>>,
) -> Result<(), String> {
    let active_loans = get_active_loans(borrows, live);

    match stmt {
        Statement::Assign(_, Rvalue::Ref(is_mut, target_vid)) => {
            check_borrow_conflict(func, Place::Local(*target_vid), *is_mut, &active_loans)?;
        }
        Statement::Assign(_, Rvalue::RefVar(is_mut, name)) => {
            check_borrow_conflict(func, Place::Param(name), *is_mut, &active_loans)?;
        }
        Statement::Assign(_, Rvalue::RefField(is_mut, target_vid, _)) => {
            check_borrow_conflict(func, Place::Local(*target_vid), *is_mut, &active_loans)?;
        }
        Statement::Assign(_, Rvalue::RefFieldVar(is_mut, name, _)) => {
            check_borrow_conflict(func, Place::Param(name), *is_mut, &active_loans)?;
        }
        _ => {}
    }

    let (def, uses) = get_stmt_defs_uses(stmt);

    for u in uses {
        check_read_access(func, u, &active_loans)?;
    }

    if let Some(d) = def {
        check_write_access(func, d, &active_loans)?;
    }

    match stmt {
        Statement::Store(var_id, _) => {
            check_deref_write_access(func, Place::Local(*var_id), borrows, live)?;
        }
        Statement::StoreVar(name, _) => {
            check_deref_write_access(func, Place::Param(name), borrows, live)?;
        }
        Statement::StoreField(var_id, _, _) => {
            check_deref_write_access(func, Place::Local(*var_id), borrows, live)?;
        }
        Statement::StoreFieldVar(name, _, _) => {
            check_deref_write_access(func, Place::Param(name), borrows, live)?;
        }
        _ => {}
    }

    Ok(())
}

fn check_term<'a>(
    func: &Function<'a>,
    term: &Terminator<'a>,
    borrows: &HashMap<Place<'a>, HashSet<Loan<'a>>>,
    live: &HashSet<Place<'a>>,
) -> Result<(), String> {
    let active_loans = get_active_loans(borrows, live);

    for u in get_term_uses(term) {
        check_read_access(func, u, &active_loans)?;
    }

    if let Terminator::Return(Some(op)) = term {
        if let Type::Ref { .. } = func.ret_type {
            if let Some(v) = get_operand_place(op) {
                if let Some(loans) = borrows.get(&v) {
                    for loan in loans {
                        if let Place::Local(local_vid) = loan.place {
                            return Err(format!(
                                "Cannot return reference to local variable '_t{}'",
                                local_vid.0
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn check_function<'a>(func: &Function<'a>) -> Result<(), String> {
    let liveness = run_liveness(func);
    let borrows_in = run_points_to(func);

    for bb in &func.basic_blocks {
        let mut current_borrows = borrows_in[&bb.id].clone();
        let (live_points, term_live) = compute_live_points(bb, &liveness.live_out[&bb.id]);

        for (i, stmt) in bb.statements.iter().enumerate() {
            check_stmt(func, stmt, &current_borrows, &live_points[i])?;
            transfer_statement(&mut current_borrows, stmt);
        }

        check_term(func, &bb.terminator, &current_borrows, &term_live)?;
    }

    Ok(())
}

pub fn check_program<'a>(program: &Program<'a>) -> Result<(), String> {
    for func in &program.functions {
        check_function(func)?;
    }
    Ok(())
}
