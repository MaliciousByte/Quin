# 🌌 Quin

Quin is a fast, hybrid-typed programming language built on a custom, language-aware execution engine written in Rust. 

> *Fast enough to matter. Simple enough to enjoy. Clean enough to last.*

---

## Overview

Modern hardware demands performance, yet legacy languages are often constrained by decades-old architectural choices. Quin is designed for the modern era—providing the flexibility of a hybrid-typed language with execution speeds that rival production-grade virtual machines.

By shedding the legacy constraints of global interpreter locks (GIL) and single-threaded event loops, Quin implements the same cutting-edge optimization techniques found in engines like V8—including NaN Boxing, Hidden Classes, and Inline Caching—but built securely and safely in Rust.

## Core Features

- **NaN Boxing:** Every value (integers, floats, booleans, null, and object pointers) is elegantly packed into a unified 8-byte representation.
- **Hidden Classes (Shapes):** Property access is optimized via direct memory offsets rather than expensive hash table lookups.
- **Inline Caching:** Repeated property reads and global accesses are seamlessly reduced to lightning-fast pointer comparisons.
- **Just-In-Time (JIT) Compilation:** Frequently executed "hot" paths are dynamically compiled to highly optimized native machine code using Cranelift.
- **Duality of Types:** Supports both inferred types and explicit annotations seamlessly within the same context, generating zero-guard native code when types are statically known.
- **Memory Efficiency:** No garbage collector, no stop-the-world pauses. Predictable memory management via scoped allocation, stack primitives, and safe Atomic Reference Counting.

---

## Architecture Design: Hotaru

Hotaru is Quin's language-aware runtime, designed to bridge high-level flexibility and native performance.

### Tiered Execution
Every piece of code enters a unified pipeline: source → AST → register-based bytecode.
- **Hotaru Core:** A baseline interpreter using direct-threaded dispatch. It employs **bytecode specialization**, rewriting instructions in place (e.g., `ADD` → `ADD_INT`) based on observed runtime types.
- **The JIT Compiler:** Hot paths are lifted into native machine code via Cranelift. Profiling data is persisted to `.hotaru` files, eliminating warm-up periods on subsequent runs.

### Memory Management
Quin uses a GC-free model to ensure zero stop-the-world pauses:
1. **Stack/Arena:** Primitives and scoped objects are allocated and freed instantly.
2. **ARC:** Only escaping objects use Atomic Reference Counting with a lightweight cycle detector.

---

## Hybrid Typing & Pipeline Duality

Quin allows annotated and inferred types to coexist seamlessly. This duality is a first-class feature of the engine:

```quin
let x = 5        // inferred → specialization handles it
let y: int = 5   // annotated → JIT is fully trusted with zero guards
```

- **The Smart Lane (Inferred):** Hotaru Core observes types at runtime and specializes instructions natively.
- **The Fast Lane (Annotated):** The JIT reads compile-time metadata and generates native code without speculative guards or checking overhead.

This dual-path optimization allows Quin to take the "fast lane" whenever types are known and the "smart lane" when they aren't—all within the same program.


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

---

## Editor Support

**VS Code / VSCodium:** Install the [Quin Language](https://open-vsx.org/extension/MaliciousByte/quin-lang) extension for syntax highlighting of `.qn` files.

Features: keyword highlighting, string and number literals, function and type names, all Quin-specific operators (`|>`, `?.`, `??`), bracket matching, and comment highlighting.

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
