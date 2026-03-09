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
- **Static Typing**: Early error detection with a robust type system.
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
    let name: str;
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
let multiplier: task(int) -> (task(int) -> int) = task(factor: int) => task(x: int) => x * factor;
let double: task(int) -> int = multiplier(2);

attempt {
    let puppy: Dog = Dog("Buddy");
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

- [**Introduction**](doc/introduction.md): Overview and Philosophy.
- [**Getting Started**](doc/getting_started.md): Installation and your first program.
- [**Syntax Basics**](doc/syntax_basics.md): Variables, types, and control flow.
- [**Collections**](doc/collections.md): Arrays, Dicts, Sets, and Tuples.
- [**Functional Programming**](doc/functional_programming.md): Tasks, closures, and pipes.
- [**Object-Oriented Programming**](doc/oop.md): Classes, traits, and shared members.
- [**Error Handling**](doc/error_handling.md): `attempt`, `rescue`, and `raise`.

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
emit("Hello, Quin!");
```

Run it instantly:
```bash
quin main.qn
```

---

## 🛣️ Roadmap

- [x] Custom Bytecode VM
- [x] Full Closure & Upvalue Support
- [x] Structured Error Handling (`attempt/rescue`)
- [x] Class & Trait System
- [x] Tracing Garbage Collector (Replacing Rc cycles)
- [x] Static Type System (Foundation & Syntax)
- [ ] Standard Library Expansion (FS, HTTP, OS)

---

**License**: MIT  
**Author**: MaliciousByte & The Quin Contributors
