# The King Programming Language

King is a statically typed programming language that builds on the core concepts of Rust (safety enforcement via borrow checker) but focuses on developer ergonomics. The shortest and cleanest way to write a block of code is always also the most performant. You can find the formal language specification under docs/spec.

The compiler is written fully in Rust. There are plans to port our front and middle end to King once the language has matured enough.

We use LLVM for the backend, specifically via the Inkwell crate. Our own LLVM API will be written before we begin the bootstrapping process.

