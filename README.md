# 🌌 Quin Programming Language

**Quin** is a modern, high-performance, statically-typed programming language designed for reliability, speed, and developer happiness. Built on top of a custom bytecode Virtual Machine (VM) implemented in Rust, Quin combines the aesthetics of modern scripting languages with the performance of systems-level execution.

---

## 🚀 Key Features

### 💎 Object-Oriented Excellence
- **Full Class System**: Inheritance, encapsulation, and polymorphism.
- **Interfaces & Abstraction**: Design robust architectures with `interface` and `abstract class`.
- **Static Members**: Powerful class-level tasks and properties.
- **Self-Binding**: Intuitive `self` reference management in constructors and methods.

### 🧩 Functional Prowess
- **First-Class Tasks**: Pass functions as arguments or return them from other functions.
- **Lambdas & Closures**: Anonymous tasks with lexical scoping.
- **Pipe Operator (`|>`)**: Clean, readable data flow chaining.

### 🛡️ Safety & Modern Syntax
- **Nullable Safety**: Optional chaining (`?.`) to prevent null-reference errors.
- **Static Typing**: Explicit type annotations with a powerful inference engine.
- **Modern Control Flow**: Ternary expressions, `if/else` blocks, and sophisticated loops.

### ⚡ Performance & Runtime
- **Bytecode VM**: A fast, stack-based environment optimized for modern CPUs.
- **Native Interop**: High-speed native function bindings for high-performance extensions.

---

## 🛠️ Quick Start

### Installation

Ensure you have Rust installed. Clone the repository and build:

```bash
git clone https://github.com/quin-lang/quin.git
cd quin
cargo build --release
```

Add the `target/release/quin` binary to your system **PATH** for global access.

### Your First Program

Save the following as `hello.qn`:

```quin
# Define a class
class Greeter {
    constructor(name: str) {
        self.name = name;
    }

    task greet() {
        emit("Hello, " + self.name + "! Welcome to Quin.");
    }
}

# Use the class with optional chaining
let myGreeter = Greeter("Developer");
myGreeter?.greet();
```

Run it instantly:
```bash
quin hello.qn
```

---

## 📖 Language Overview

### Variables & Scoping
Quin uses `let` for immutable and mutable variables, supporting block-level scoping.
```quin
let x = 10;
let mut y = 20; # If mutability is enabled
```

### Collections
Native support for powerful data structures:
- **Dicts**: `let map = {"key": "value"}`
- **Sets**: `let s = set {1, 2, 3}`
- **Tuples**: `let t = (1, "two", 3.0)`

### The Pipe Operator
Elegant data transformations:
```quin
let results = [1, 2, 3] 
    |> map(task(x) { x * 2 })
    |> filter(task(x) { x > 2 });
```

---

## 🛣️ Roadmap

- [x] Custom Bytecode VM
- [x] OOP Support (Classes, Methods, Inheritance)
- [x] Functional Features (Lambdas, Pipes)
- [ ] Tracing Garbage Collector (Replacing Rc cycles)
- [ ] Full Static Type Checker
- [ ] Standard Library Expansion (FS, HTTP, OS)

---

## 🛰️ Community & Contributing

Quin is open-source. We welcome contributions to the compiler, VM, and standard library.

**License**: MIT
**Author**: Quin Team
