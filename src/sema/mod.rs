#![allow(unused_imports)]

pub mod ast;
pub mod context;
pub mod generics;
pub mod comptime;
pub mod statement;
pub mod expr;
pub mod analyze;

pub use ast::*;
pub use context::{StructMeta, FunctionMeta, mangle_name, SemaContext, OptionExt};
pub use generics::{
    type_to_hir, substitute_type, substitute_statement, substitute_block, substitute_expr,
    get_mangled_mono_name, resolve_generic_template,
};
pub use comptime::{ComptimeVal, eval_comptime_expr, eval_comptime_block};
pub use statement::{check_statement, check_block};
pub use expr::{check_expr, is_writable};
pub use analyze::{analyze, get_type_id};
