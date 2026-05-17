# **King Programming Language: High-Level Intermediate Representation (HIR) Specification**

This document formally defines the High-Level Intermediate Representation (HIR) for the King Programming Language. Based on the King compiler pipeline, the HIR serves as the critical bridge between the parsed, imported-resolved Abstract Syntax Tree (AST) and the Sema (semantic analysis and type-checking) phase. The HIR is initially non-typed but structurally rigid, heavily optimized for the Sema module to perform borrow-checking annotations, monomorphization, and type inference.

## **1. Architectural Scope and Desugaring**

The HIR builder translates the AST into a normalized format. Crucially, the King specification dictates specific transformations that must occur before or during HIR construction:

* **Argument Canonicalization:** Named arguments and default parameters are resolved into strict positional vectors. The HIR function call node has no concept of named arguments.  
* **Path Resolution:** All symbol paths (imports, module-level variables) are fully resolved to unique DefId identifiers by the Resolver prior to HIR construction.  
* **Untyped to Typed Pathway:** The HIR node structures contain TypeRef slots that are populated with Unresolved markers. The Sema module will traverse the HIR and replace these with concrete types.

## **2. Core Identifiers and Types**

Every node in the HIR is tracked via a unique identifier to allow the Sema, MIR builder, and Static Analysis tools to map metadata (like borrow lifetimes and types) back to the exact code location without mutating the tree.

struct HirId(u32);  
struct DefId(u32); // Canonical ID from the Resolver

enum TypeRef {  
    Unresolved,  
    Primitive(PrimitiveType),  
    Table(DefId),  
    TraitBound(DefId),  
    VariadicOthers(TypeLayout), // For the 'others' keyword  
    // Populated by Sema:  
    Inferred(ConcreteTypeId)   
}

## **3. Top-Level Items**

The root of the HIR is the Module, which contains a collection of Item nodes. These encompass the core structural components of King.

| Item Kind | HIR Structure | Semantics & Checks   |
| :---- | :---- | :---- |
| **Table** | ItemTable { id: DefId, fields: Vec<FieldDef> } | Enforces First-Class SoA constraints. Sema will reject any field types containing references, dynamic vectors, or unsized elements. |
| **Trait** | ItemTrait { id: DefId, methods: Vec<MethodSig> } | Structural interfaces only. No explicit implementation mappings. |
| **Function** | ItemFn { sig: Signature, body: Block } | Stores generic type gates, value contracts, and given capabilities. |

### **3.1 Function Signatures and Contracts**

King's advanced signature attributes, such as `given` capabilities, in-signature contracts, and the `others` keyword, are explicitly captured in the HIR.

struct Signature {  
    name: Name,  
    params: Vec<Param>,  
    has_others: bool,         // Flag for the variadic 'others' keyword  
    return_type: TypeRef,  
    capabilities: Vec<Capability>, // Extracted from the `given` keyword  
    type_gates: Vec<Expr>     // Compile-time gates [ T.is_numeric() ]  
}

struct Param {  
    name: Name,  
    ty: TypeRef,  
    value_contract: Option<Expr> // Runtime value contracts [ != 0.0 ]  
}

## **4. Statements and Error Routing**

The HIR models statements independently to capture King's unique flow control, particularly its ergonomic error routing and unrecoverable assertions.

* **Let Binding:** Standard variable assignment. Let(Pat, Expr)  
* **Assertive Unwrap (!let):** Maps the !let operator, directing the Sema to enforce an immediate panic branch if the RHS evaluates to an error. AssertiveLet(Pat, Expr)  
* **Expression Statement:** An expression evaluated for side effects. ExprStmt(Expr)

## **5. King-Specific Expressions**

The core of the HIR resides in its custom expression types, mapped directly to King's unique paradigms: queries, hardware assertions, context injection, and comptime execution.

enum ExprKind {  
    // 5.1 Data-Oriented Queries  
    Query(QueryNode),  
    MultiQuery(Vec<QueryNode>),

    // 5.2 Error Routing (handle let)  
    HandleLet {  
        target: Box<Expr>,  
        ok_arm: PatArm,  
        err_arm: PatArm,  
        pass_through: bool, // True if 'ok!' syntax is used  
    },

    // 5.3 Isolation and Memory  
    TrustingBlock {  
        category: TrustCategory, // FFI, Ptr, Borrow  
        body: Block  
    },  
    ArenaBlock(Block), // Marks region allocation lifetime root

    // 5.4 Comptime and Context  
    ComptimeBlock(Block), // Sent to monomorphization for reflection eval  
    WithContext {  
        capabilities: Vec<Assignment>, // e.g., allocator = global_pool  
        body: Block  
    },

    // Standard Constructs  
    Call(Box<Expr>, Vec<Expr>), // Positional only (defaults resolved)  
    InlineFor(Pat, Box<Expr>, Block),  
    BinaryOp(BinOp, Box<Expr>, Box<Expr>),  
    Path(DefId),  
}

### **5.1 The Query Subsystem Node**

Queries are native looping constructs that decompose SoA structures. The HIR must isolate the where constraints, the select access pathways, and the execution block to allow the Static Analyzer to determine thread-pool splitting viability.

struct QueryNode {  
    target_table: DefId,  
    where_clause: Option<Expr>,  
    select_fields: Vec<QuerySelect>,  
    body: Block  
}

struct QuerySelect {  
    field_id: DefId,  
    is_mut: bool // Dictates exclusive vs shared borrow tracking  
}