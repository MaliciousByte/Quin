# 🌌 Quin Programming Language

**Quin** is a modern, high-performance programming language designed for reliability, speed, and developer happiness. Built on a custom bytecode VM in Rust, Quin combines the elegance of modern syntax with the performance of systems-level execution.

---

## 🚀 Key Features

### 💎 Object-Oriented Excellence
- **Full Class System**: Inheritance, encapsulation, and polymorphism using `base` classes.
- **Traits (Interfaces)**: Design robust architectures with the `trait` and `with` system.
- **Shared Members**: Replace static bloat with the `shared` keyword for methods and properties.
- **Refined Constructors**: Intuitive initialization using the `init` keyword.

### 🧩 Functional Prowess
- **First-Class Tasks**: Pass functions as arguments or return them from other tasks.
- **Closures & Upvalues**: Anonymous tasks with full lexical scoping and variable capture.
- **Arrow Shorthand**: Clean syntax for simple tasks: `task add(a, b) => a + b;`.
- **Pipe Operator (`|>`)**: Clean, readable data flow chaining.

### 🛡️ Safety & Modern Syntax
- **Flexible Typing**: Dynamically typed with support for optional type annotations for better clarity.
- **Null Safety**: Optional chaining (`?.`) and Nullish Coalescing (`??`) to handle voids gracefully.
- **String Interpolation**: Embed expressions directly: `"Hello {name}!"`.
- **Advanced Match**: Pattern matching with ranges and default cases.
- **Error Handling**: Robust `attempt / rescue / finally` blocks for structured error recovery.

---

## 🛠️ Quick Start

### Installation

Ensure you have Rust installed. Clone the repository and build:

```bash
cargo build --release
```

Add the `target/release/quin` binary to your system **PATH** for global access.

### Your First Program

Save the following as `main.qn`:

```quin
trait Sound {
    task make_noise();
}

class Animal {
    let name;
    init(name) {
        self.name = name;
    }
}

class Dog extends Animal with Sound {
    task make_noise() {
        emit("Dog {self.name} says: Woof!");
    }
}

# Use closures and advanced syntax
let multiplier = task(factor) => task(x) => x * factor;
let double = multiplier(2);

attempt {
    let puppy = Dog("Buddy");
    puppy.make_noise();
    emit("Double 21 is: {double(21)}");
} rescue (e) {
    emit("Error encountered: {e}");
} finally {
    emit("Execution complete.");
}
```

Run it instantly:
```bash
quin main.qn
```

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
- [ ] Module System & Package Manager
- [ ] Async Runtime
- [ ] Network & HTTP Library

---

**License**: MIT  
**Author**: MaliciousByte & The Quin Contributors

