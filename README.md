## The King Programming Language

King is a statically typed programming language that builds on the core concepts of Rust (safety enforcement via borrow checker) but focuses on developer ergonomics. The shortest and cleanest way to write a block of code is always also the most performant. You can find the formal language specification under docs/spec.

The compiler is written fully in Rust. There are plans to port our front and middle end to King once the language has matured enough.

We use LLVM for the backend, specifically via the Inkwell crate. Our own LLVM API will be written before we begin the bootstrapping process.

---

## The compiler layout

In order to ensure modularity across the compiler, we split every big task into its own module. The pipeline looks like this:
 
- Lexer: emits tokens
- Parser: build an AST
- Resolver: handle imports, etc, adds to AST
- HIR builder: build a non-typed HIR from the AST
- Sema: perform type checking and inference, emit typed HIR
- MIR builder: build "naive" MIR
- Optimizer: perform DCE, constant folding, etc.
- Static Analysis: borrow checking, automatic drop insertion on cleaned-up MIR
- Codegen: builds final LLVM IR

Helpers: 
- Error module: Used by all others for coherent error messages.
- Main module: Glues everything together, handles source I/O.