# **The King Programming Language Specification**

---

**Spec Version:** 0.1.0   
**Paradigm:** Compile-Time Meta-Programming, Zero-Cost Memory Safety, First-Class Structure-of-Arrays (SoA) Data-Oriented Design, Type-Inferred Region Memory Management.

## **1. Executive Design Philosophy**

---

King addresses systemic developer friction points in traditional systems programming languages while retaining guaranteed memory safety without a garbage collector. The language builds directly on the borrow checker’s lifetime topology, upgrading it from a verification system into a proactive optimization backend.

## **2. Type System & First-Class SoA Tables**

### ---

**2.1 Primitive Restrictions on Table Items**

A table is a collection of uniform instances structured as a Structure of Arrays (SoA). To guarantee perfect memory contiguity and deterministic hardware cache behaviors, elements stored inside tables must adhere to strict layout limits.

* **Allowed Types:** Fixed-width integers (i8 through i128, u8 through u128), floating-point numbers (f32, f64), booleans, fixed-size arrays of primitives, and child structs consisting solely of these types.  
* **Forbidden Types:** Heap-allocated references (strings, dynamic vectors), raw pointers, or type definitions with unsafe self-referential bounds.

### **2.2 Table Declaration Syntax**

```
table Entities {  
    id: u64,  
    position_x: f64,  
    position_y: f64,  
    velocity_x: f64,  
    velocity_y: f64,  
    active: bool,  
}
```
### **2.3 Query & Multiquery Expression Blocks**

Queries act as native compiler loops that break down structural inputs into isolated vectors. The syntax forces developers to specify exact mutation pathways up front, feeding directly into compile-time borrow validations.

```
query Entities {  
    where { .active == true && .position_x > 0.0 }  
    select { mut position_x; velocity_x }  
    {  
        .position_x += .velocity_x * 0.016;  
    }  
}
```
A multiquery block combines multiple independent query loops. If the compiler verifies that no two query blocks concurrently demand exclusive (mut) access to overlapping fields, it splits indices across thread pools automatically.

```
multiquery Entities {  
    query Move {  
        where { .active == true }  
        select { mut position_x; velocity_x }  
        { .position_x += .velocity_x; }  
    }  
    query RenderCheck {  
        where { .active == true }  
        select { position_y }  
        {   
            // Read-only access allowed simultaneously  
            log_position(.position_y);  
        }  
    }  
}
```
## **3. Function Signatures & Advanced Arguments**

### ---

**3.1 Elegant Variadics via the others Keyword**

The others keyword represents an arbitrary sequence of function inputs. Depending on signature constraints, it monomorphizes into either a homogenous stack slice or a heterogenous compiled tuple structure.

```
fn log_metrics(category: u32, others: f64) {  
    inline for i in 0..others.len {  
        emit_raw_metric(category, others[i]);  
    }  
}
```
Generic functions can also utilize variadics.

```
fn multiAdd<T [is i64 || is f64]>(first: &str, others: T) -> T {...}
```
In this case, every single item in `others` must be of type T.

### **3.2 Named and Default Arguments**

Function arguments can contain default fallback assignments. Any argument featuring a default value, along with all following inputs in that signature, must be provided as a named token at the call site if changed from default.

```
fn configure_core(frequency: u32, voltage: f64 = 1.2, turbo: bool = false) { ... }

// Permitted call permutations  
configure_core(4000);  
configure_core(4200, turbo: true);  
configure_core(3800, voltage: 1.15, turbo: false);
```
The compiler parses named calls by looking up missing defaults and re-ordering properties into positional indices at the Abstract Syntax Tree (AST) level before compiling down further.

### **3.3 In-Signature Contract Constraints**

Value and type gates are attached to parameter fields inside brackets [ ... ]. This syntax uniformly manages runtime arithmetic rules and compile-time generic criteria.

| Constraint Category | Syntax Prototype | Execution Strategy   |
| :---- | :---- | :---- |
| **Runtime Value Contract** | fn scale(v: f64 [ != 0.0 ]) | Value range propagation; elides checks if provable, inserts fast call-site panic branch if dynamic. |
| **Compile-Time Type Gate** | fn run<T>(a: T [ is i64 || is i32 ]) | Evaluated entirely during monomorphization. Emits structured compiler error if condition is false. |

## **4. Complete Comptime Reflection & Evaluation**

---

The language implements a comptime execution pipeline where normal code logic can execute natively inside the compiler shell. This completely replaces standard structural macro logic with programmatic introspection APIs.

```
fn generate_serializer<T>() {  
    comptime {  
        let metadata = reflect::();  
        inline for field in metadata.fields {  
            if field.type.is_numeric() {  
                println!("Compiling numeric serialization hook for {}", field.name);  
            }  
        }  
    }  
}
```
Standard library schema parsing (such as JSON or database definitions) utilizes this subsystem. By passing a file string to a comptime block, developers inject generated data layouts into the global type registry with zero runtime performance impacts.

## **5. Flow Control & Ergonomic Error Routing**

### ---

**5.1 Structural Error Routing via handle let**

The handle let construct unifies allocation bounds checking and failure resolution into a clean, un-nested layout block.

```
handle let compute_node = safe_divide(val, step) {  
    ok => apply_transformation(compute_node),  
    err => fallback_execution()  
}
```
### **5.2 Assertive Unwrapping via !let**

For operations where failure indicates an unrecoverable system invariant, the ! operator can prefix assignments to force immediate call-site panic handling.

```
!let critical_io = active_device_bus.mount_channel(0);
```
### **5.3 The Hybrid ok! Pass-Through Escape**

When error branches require isolated handling, but successful extractions should remain un-nested, the ok! token allows values to flow straight into the enclosing scope block.

```
handle let resource = parse_system_descriptor(id) {  
    ok!,  
    err => {  
        emit_diagnostic_dump();  
        return; // Diverges safely  
    }  
}  
// resource is fully initialized and directly accessible here  
use_resource(resource);
```
## **6. Structural Traits & Interface Validation**

---

Traits in King are defined structural interfaces checked at compile time via reflection predicates, completely decoupling implementation logic from third-party types.

```
trait Writer {  
    fn write_buffer(&mut self, data: &[u8]) -> Result;  
}
```
`impl ... for ...` can then be used to implement that trait per type.
There can be multiple type (or struct) names after the `for` to boundle implementation where it is identical. You then use standard generics to write the code:

```
trait MathCapable {
    fn square(&mut self);
    fn double(&mut self);
}
impl MathCapable for i32, i64, f32, f64 {
    // constraints automatically enforced
    fn square<T>(&mut self: T) { return self * self; }
    fn double<T>(&mut self: T) { return self * 2 as T; }
}
```

## **7. Auditable Isolation: The trusting Subsystem**

---

The trusting keyword isolates code segments that step past standard validation parameters. It requires developers to label the exact engineering safety check they are bypassing.

* trusting ffi: Enables raw foreign binary execution lines while keeping local variable borrow tracking active.  
* trusting ptr: Unlocks manual memory pointer offsets and volatile register access commands.  
* trusting borrow: Grants temporary clearance to override compiler reference aliasing checks when overlapping segments can be proven safe manually.

```
trusting borrow {  
    let segment_a = mut hardware_matrix[0];  
    let segment_b = mut hardware_matrix[1];  
    intertwine(mut segment_a, mut segment_b);  
}
```
## **8. Lifetime-Inferred Region Allocators**

---

Because the compiler's borrow checker evaluates the absolute lifecycle topology of every reference, it uses this information to optimize allocation patterns automatically via the arena keyword.

```
arena {  
    let context_root = BuildNode::init();  
    for active_token in parsing_stream {  
        let element = parsed_node(active_token);  
        context_root.attach(element);  
    }  
} // The entire underlying arena block memory region is released instantly here
```
When objects are declared inside an arena context block, individual allocations bypass standard global locks, resolving instead via high-speed register bump updates. Destructors for types without system resource hooks (like files or sockets) are completely omitted from the binary, turning cleanup routines into a single, instantaneous memory release.

## **9. Context & Colorless Async Capabilities**

### ---

**9.1 Colorless Async Lifecycles**

Asynchronous operations do not change the base function signature or require .await markers. Functions are written using standard logic. The calling context determines how execution routes.

```
fn acquire_payload(node_id: u64) -> Payload {  
    let network_stream = network::connect(node_id);  
    network_stream.read_payload()  
}
```
When invoked from a normal thread, execution blocks. When executed within an async_pool context, the compiler generates a non-blocking state machine under the hood using monomorphization paths.

### **9.2 Context Capabilities via given and with**

To eliminate signature clutter from tracking environmental settings like logging layers or memory contexts, capabilities are defined as implicit contextual values handled by the compiler.

```
fn compile_subtask() given (allocator: mut RegionArena, diagnostics: Logger) {  
    let work_buffer = allocator.claim_segment(2048);  
    diagnostics.write("Subtask workspace allocated.");  
}
```
Call blocks provide the concrete values to fulfill those dependencies safely without explicit signature passing.

```
with (allocator = global_bump_pool, diagnostics = system_std_out) {  
    compile_subtask();  
}
```