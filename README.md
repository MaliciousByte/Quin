# 🌌 Quin Programming Language

**Quin** is a dynamically typed, high-performance programming language built on a custom two-tier VM written in Rust. It starts interpreting immediately like Python, and compiles hot code to native machine instructions at runtime — no manual compilation step, no waiting.

It was built to answer one question:
> *Why do I have to choose between writing code that feels good and code that runs fast?*

Python feels good but crawls under load. JavaScript is fast but single-threaded and "25 years of weird." Quin is neither.

---

## ⚡ The Dynamic Fast Path

Most dynamic languages accept a "speed tax" — the cost of figuring out types at runtime. Quin eliminates this tax with four techniques used together in a Rust-native VM. No other language currently combines all four in a clean-slate design.

### 1. NaN Boxing — Everything is 8 Bytes
In Python, every variable is a heavy C struct carrying type information, reference counts, and the actual value — often 28+ bytes per value. In Quin, every value — integers, floats, booleans, nulls, and object pointers — is packed into a single 64-bit float using the IEEE 754 NaN bit space.
**The result:** every value on the stack is exactly 8 bytes, fits in one CPU register, and requires zero "unwrapping" to read.

### 2. Hidden Classes (Shapes) — No More Dictionary Lookups
Quin assigns every object a **Shape** — a shared map that says "for any object like this, name is always at memory position 0." Instead of searching a hash map (like Python/JS), the VM jumps directly to the exact memory address. Objects with identical property layouts share the same Shape automatically.

### 3. Inline Caching — The Shortcut That Remembers
When the VM reads `user.name` for the first time, it stores a note at that specific instruction: *"last time, this was Shape #5 and name was at index 0."* Next time, it checks — still Shape #5? Skip everything, read index 0 directly. A full property lookup collapses to a single pointer comparison.

### 4. Cranelift JIT — Native Machine Code at Runtime
The VM watches every function. Once something is called 1,000+ times, it's marked "hot". **Cranelift** — a Rust-native code generation backend — compiles that function directly to machine code while the program is running. The next call skips the interpreter entirely.

---

## 💎 Language Features

Beyond the engine, Quin is designed for modern development:

- **Object-Oriented**: Full Class system with `base` inheritance, `trait` contracts, and `shared` members.
- **Functional**: First-class `tasks`, full lexical closures, and the pipe operator (`|>`).
- **Modules**: Native support for `use` imports, structured exports, and circular dependency resolution.
- **Modern Syntax**: Null safety (`?.`, `??`), string interpolation, and advanced pattern matching.
- **Error Handling**: Structured `attempt / rescue / finally` blocks with stack-aware propagation.

---

## 📊 Quin vs Python vs JavaScript

| Workload | Python (CPython) | JavaScript (V8) | Quin |
| :--- | :--- | :--- | :--- |
| **Cold Startup** | 🐢 Slow | ⚡ Fast | ⚡ Fast |
| **Simple Scripts** | 🐢 1x baseline | 🚀 ~50x Python | 🚀 ~50x Python |
| **Heavy Math Loops** | 🐢 Slow | 🚀 Fast (JIT) | 🚀 Fast (JIT) |
| **Multi-core Parallel** | 🔴 GIL blocks it | 🔴 Single-threaded | ✅ True parallelism |
| **Long-running Apps** | 🟡 Acceptable | 🟡 GC stutters | ✅ No GC pauses |
| **Memory per Value** | ~28 bytes | 8 bytes | 8 bytes |

### Parallelism
Python has the GIL — only one thread runs Python code at a time. JavaScript has the event loop — fundamentally single-threaded. **Quin is backed by Rust. No GIL. Real threads.** If you have 8 cores, Quin can use all 8 simultaneously.

### Memory Management
JavaScript GC pauses cause micro-stutters. Python's cyclic GC freezes execution. **Quin cleans memory incrementally** using a mix of RC and Rust ownership — the program never stops to "take out the trash."

---

## 📜 Legacy Baggage

**JavaScript** was created in 10 days in 1995. It carries 30 years of quirks — `typeof null === "object"`, implicit coercions, `var` hoisting, and `this` context madness.

**Python** carries C-era design decisions and the GIL — a lock added in 1992 that restricts modern hardware.

**Quin is a clean slate.** No design decisions made before the internet existed. No backwards compatibility with 1995.

---

## 🛠️ Quick Start

### Your First Program

Run Quin instantly by passing a file to the CLI:
```bash
quin main.qn
```

Or dive into the documentation to start building.

---

## 📖 Documentation

Explore the full capabilities of Quin:

- [**Introduction**](docs/introduction.md): Overview and Philosophy.
- [**Getting Started**](docs/getting_started.md): Installation and your first program.
- [**Syntax Basics**](docs/syntax_basics.md): Variables, types, and control flow.
- [**Collections**](docs/collections.md): Arrays, Dicts, Sets, and Tuples.
- [**Functional Programming**](docs/functional_programming.md): Tasks, closures, and pipes.
- [**Object-Oriented Programming**](docs/oop.md): Classes, traits, and shared members.
- [**Error Handling**](docs/error_handling.md): `attempt`, `rescue`, and `raise`.
- [**Standard Library**](docs/stdlib.md): Built-in functions (math, string, array, IO, OS).

---

## 💻 Interactive REPL

Run `quin` without arguments to launch the interactive shell:

```
  ╔══════════════════════════════════════╗
  ║   🌌 Quin v0.1.0                     ║
  ║   Interactive Mode                   ║
  ╚══════════════════════════════════════╝

  Type .help for commands, .exit to quit.

>>> let x = 42;
>>> emit(x * 2);
84
>>> task greet(name) { emit("Hello {name}!"); }
>>> greet("World");
Hello World!
```

Multi-line input is supported — open a `{` and keep typing.

---

## 🛣️ Roadmap

- [x] Custom Bytecode VM
- [x] Full Closure & Upvalue Support
- [x] Structured Error Handling (`attempt/rescue`)
- [x] Class & Trait System
- [x] String Interning & NaN-Boxing
- [ ] Static Type System (Type Checking & Enforcement)
- [x] Dynamic VM Core with Type-aware Values
- [x] Interactive REPL
- [x] Standard Library (Math, String, Array, IO, OS)
- [x] Module System
- [ ] Package Manager
- [ ] Async Runtime
- [ ] Network & HTTP Library

---

## 🛠️ Build and Release

To compile Quin from source for distribution:

1.  **Clone the Repo**: `git clone https://github.com/MaliciousByte/Quin.git`
2.  **Build Release Binary**:
    ```bash
    cargo build --release
    ```
3.  **Find the Binary**:
    The standalone executable will be at `target/release/quin.exe` (Windows) or `target/release/quin` (macOS/Linux).

---

**License**: MIT  
**Author**: MaliciousByte & The Quin Contributors