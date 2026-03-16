# 🌌 Quin

Quin is a dynamically typed programming language built on a custom bytecode VM written in Rust. It interprets code immediately and compiles hot functions to native machine code at runtime via a Cranelift JIT compiler.

> *Fast enough to matter. Simple enough to enjoy. Clean enough to last.*

---

## Why Quin

Every major dynamic language carries decades of decisions made before modern hardware existed.

Python's GIL was added in 1992 — before multi-core CPUs were standard. One lock, one core, always. JavaScript was written in 10 days in 1995 for a browser. Its single-threaded event loop is a fundamental constraint, not a bug to be fixed. Both languages have grown enormous ecosystems around these limitations, making them impossible to remove without breaking everything.

Quin starts in 2026 with full knowledge of what those decisions cost. No GIL. No event loop. No 30-year-old design choices baked into the foundation. A clean VM built on Rust, using the same optimization techniques as V8 — NaN Boxing, Hidden Classes, Inline Caching, JIT compilation — but without the legacy that makes V8 so difficult to change.

It is not finished. But the foundation is being built correctly.

---

## What Quin Is

Quin is a **work-in-progress language** with a production-grade VM core. The runtime is built from scratch in Rust with four optimization techniques working together:

- **NaN Boxing** — every value (int, float, bool, null, object pointer) packed into 8 bytes
- **Hidden Classes (Shapes)** — property access via direct memory offset, not hash lookup
- **Inline Caching** — repeated property reads reduced to a single pointer comparison
- **Cranelift JIT** — functions called 1000+ times compiled to native machine code

These are the same techniques used in V8 (JavaScript). Quin implements all four in a clean-slate Rust VM with no garbage collector and no legacy constraints.

---

## Performance

On integer loop benchmarks with JIT warmed, Quin matches V8 (Node.js):

| Benchmark | Python (CPython) | Node.js (V8) | Quin JIT |
|-----------|-----------------|--------------|----------|
| 10M integer loop | ~460ms | ~9ms | ~7ms |

Quin is **~65x faster than CPython** and **on par with V8** for this workload. JIT coverage is expanding — property access, floats, and closures are next.

---

## Current State

Quin is at an **early but functional stage**. The VM is correct, the test suite passes, and the core optimizations are implemented. On integer-heavy workloads the JIT matches Node.js (V8) performance. General benchmark parity is in progress as JIT coverage expands.

**What works today:**

- Full bytecode compiler (lexer → parser → AST → bytecode)
- Interpreter with NaN-boxed value stack
- Hidden Classes and Inline Caching for object property access
- JIT compilation via Cranelift — integer arithmetic, control flow, all local variables
- Deoptimization — JIT bails back to interpreter cleanly on type mismatches
- OSR (On-Stack Replacement) — hot loops switch to native code mid-execution
- String interning via `StringInterner`
- Module system — `use math;` and `use { sqrt } from math;`
- Circular import detection and protection
- Full OOP — classes, inheritance, structs, closures
- Standard library — math, string, array, io, os modules
- Interactive REPL
- VS Code / VSCodium syntax highlighting

**What is in progress:**

- JIT property access (hot `obj.name` reads still interpreted)
- Type feedback vectors (needed for speculative compilation)
- True parallelism (Arc migration done, parallel runtime not yet built)
- Async / await runtime
- Package manager (Quill)

---

## Quick Start

```bash
git clone https://github.com/MaliciousByte/Quin.git
cd Quin
cargo build --release
./target/release/quin examples/hello.qn
```

**Hello World:**
```quin
emit("Hello, World!");
```

**Functions:**
```quin
task fib(n) {
    if n < 2 { return n; }
    return fib(n - 1) + fib(n - 2);
}

emit(fib(10));
```

**Modules:**
```quin
use math;
use { map, filter } from array;

let nums = [1, 2, 3, 4, 5];
let squares = map(nums, task(x) => x * x);
emit(squares);
```

---

## VM Architecture

Quin's execution pipeline:

```
Source
  → Lexer       (text → tokens)
  → Parser      (tokens → AST)
  → Compiler    (AST → bytecode chunks)
  → Interpreter (bytecode → execution)
  → Profiler    (counts calls silently)
  → JIT         (hot functions → native code via Cranelift)
  → Deopt       (type mismatch → back to interpreter at exact IP)
```

**Memory model:** No garbage collector. Reference counting via Rust's `Arc<T>`. Memory freed the instant the last reference drops. No stop-the-world pauses.

---

## Editor Support

**VS Code / VSCodium:** Install the [Quin Language](https://open-vsx.org/extension/MaliciousByte/quin-lang) extension for syntax highlighting of `.qn` files.

Features: keyword highlighting, string and number literals, function and type names, all Quin-specific operators (`|>`, `?.`, `??`), bracket matching, and comment highlighting.

---

## Roadmap

- [x] Bytecode VM
- [x] NaN Boxing
- [x] Hidden Classes and Inline Caching
- [x] String Interning
- [x] Closures and Upvalues
- [x] Class and Inheritance System
- [x] Cranelift JIT (integer arithmetic, control flow, all locals)
- [x] Deoptimization
- [x] OSR — On-Stack Replacement
- [x] Module System with circular import protection
- [x] Standard Library (math, string, array, io, os)
- [x] Interactive REPL
- [x] VS Code / VSCodium syntax highlighting
- [ ] JIT — property access
- [ ] JIT — type feedback and speculation
- [ ] True parallelism (Arc foundation ready)
- [ ] Async / await runtime
- [ ] Package manager (Quill)
- [ ] Language server (LSP)
- [ ] Static type checker (optional)

---

## Contributing

Quin is early stage and every contribution matters. The codebase is intentionally readable — if you know Rust and are interested in compilers, VMs, or language design, there is meaningful work to do at every level.

**Good first areas:**

- Expanding JIT opcode coverage in `jit.rs`
- Adding stdlib functions to existing modules
- Writing `.qn` test cases that expose edge cases
- Documentation improvements

**Before contributing:**

1. Read through `value.rs`, `vm.rs`, and `jit.rs` to understand the core architecture
2. Run the test suite — `cargo build --release` then `.\tests\run_all.ps1`
3. Open an issue before starting large changes so effort isn't duplicated

All contributions must pass the full test suite with zero compiler warnings on release build.

---

## Building from Source

Requirements: Rust toolchain (stable)

```bash
cargo build --release
```

Binary at `target/release/quin` (Linux/macOS) or `target/release/quin.exe` (Windows).

---

## Documentation

- [Introduction](docs/introduction.md)
- [Getting Started](docs/getting_started.md)
- [Syntax Basics](docs/syntax_basics.md)
- [Collections](docs/collections.md)
- [Functional Programming](docs/functional_programming.md)
- [Object-Oriented Programming](docs/oop.md)
- [Error Handling](docs/error_handling.md)
- [Standard Library](docs/stdlib.md)

---

**License:** MIT  
**Author:** MaliciousByte  
**Status:** Active development — contributions welcome
