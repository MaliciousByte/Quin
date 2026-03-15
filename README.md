# 🌌 Quin

Quin is a dynamically typed programming language built on a custom bytecode VM written in Rust. It interprets code immediately and compiles hot functions to native machine code at runtime via a Cranelift JIT compiler.

> *Fast enough to matter. Simple enough to enjoy. Clean enough to last.*

---

## What Quin Is

Quin is a **work-in-progress language** with a production-grade VM core. The runtime is built from scratch in Rust with four optimization techniques working together:

- **NaN Boxing** — every value (int, float, bool, null, object pointer) packed into 8 bytes
- **Hidden Classes (Shapes)** — property access via direct memory offset, not hash lookup
- **Inline Caching** — repeated property reads reduced to a single pointer comparison
- **Cranelift JIT** — functions called 1000+ times compiled to native machine code

These are the same techniques used in V8 (JavaScript). Quin implements all four in a clean-slate Rust VM with no garbage collector and no legacy constraints.

---

## Current State

Quin is at an **early but functional stage**. The VM is correct, the test suite passes, and the core optimizations are implemented. It is not yet production-ready and is not faster than Node.js on general benchmarks. The JIT is being expanded incrementally.

What works today:

- Full bytecode compiler (lexer → parser → AST → bytecode)
- Interpreter with NaN-boxed value stack
- Hidden Classes and Inline Caching for object property access
- JIT compilation via Cranelift for hot functions (integer arithmetic, control flow)
- Deoptimization — JIT bails back to interpreter cleanly on type mismatches
- String interning via `StringInterner`
- Module system — `use math;` and `use { sqrt } from math;`
- Full OOP — classes, inheritance, structs, closures
- Standard library — math, string, array, io, os modules
- Interactive REPL

What is in progress:

- JIT property access (hot `obj.name` reads still interpreted)
- Type feedback vectors (needed for speculative compilation)
- True parallelism (Arc migration done, parallel runtime not yet built)
- Async / await runtime
- Package manager

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

**Functions and control flow:**
```quin
task fib(n) {
    if n < 2 { return n; }
    return fib(n - 1) + fib(n - 2);
}

emit(fib(10));
```

**Classes:**
```quin
class Animal {
    task init(name) {
        self.name = name;
    }
    task speak() {
        emit(self.name + " makes a sound.");
    }
}

class Dog extends Animal {
    task speak() {
        emit(self.name + " barks.");
    }
}

let d = Dog("Rex");
d.speak();
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

## Language Reference

### Variables
```quin
let x = 42;
let name = "Quin";
let flag = true;
```

### Control Flow
```quin
if x > 0 {
    emit("positive");
} elif x == 0 {
    emit("zero");
} else {
    emit("negative");
}

while x > 0 {
    x = x - 1;
}

for item in [1, 2, 3] {
    emit(item);
}
```

### Functions and Closures
```quin
task add(a, b) {
    return a + b;
}

let double = task(x) => x * 2;

# Pipe operator
let result = 5 |> double;
emit(result);
```

### Error Handling
```quin
attempt {
    let data = read_file("missing.txt");
} rescue e {
    emit("Error: " + e);
} finally {
    emit("Done.");
}
```

### Standard Library

| Module | Key Functions |
|--------|--------------|
| `math` | `sqrt`, `pow`, `abs`, `floor`, `ceil`, `round`, `min`, `max`, `PI`, `E` |
| `string` | `upper`, `lower`, `trim`, `split`, `contains`, `replace`, `starts_with`, `ends_with` |
| `array` | `push`, `pop`, `slice`, `sort`, `range`, `map`, `filter`, `join` |
| `io` | `input`, `read_file`, `write_file`, `type_of`, `assert` |
| `os` | `clock`, `exit`, `env`, `args` |

Core functions available without import: `emit`, `len`, `type_of`, `assert`

---

## Interactive REPL

Run `quin` with no arguments:
```
  ╔══════════════════════════════════════╗
  ║   🌌 Quin v0.1.0                     ║
  ║   Interactive Mode                   ║
  ╚══════════════════════════════════════╝

>>> let x = 10;
>>> emit(x * 2);
20
```

Commands: `.help`, `.exit`, `.clear`

---

## Roadmap

- [x] Bytecode VM
- [x] NaN Boxing
- [x] Hidden Classes and Inline Caching
- [x] String Interning
- [x] Closures and Upvalues
- [x] Class and Inheritance System
- [x] Cranelift JIT (integer arithmetic, control flow)
- [x] Deoptimization
- [x] Module System
- [x] Standard Library
- [x] Interactive REPL
- [ ] JIT — property access
- [ ] JIT — type feedback and speculation
- [ ] True parallelism (Arc foundation ready)
- [ ] Async / await runtime
- [ ] Package manager
- [ ] Language server (LSP)
- [ ] Static type checker (optional)

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