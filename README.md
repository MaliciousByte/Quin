![Alt text](https://cdn.discordapp.com/attachments/1478339860484853885/1480207462735937739/ChatGPT_Image_Mar_8_2026_05_44_49_PM.png?ex=69aed620&is=69ad84a0&hm=a6318dc72d10199889bb18d7144b233a67f8b6981ae47ff527dea83dfc4e4ba8)


# Quin Programming Language

Quin is a modern, high-performance programming language implemented in Rust, featuring a custom bytecode Virtual Machine (VM).

## Features

- **Variables & Consts**: Robust scoping with `let` and `let const`.
- **Functions**: Task definitions (`task`) with support for parameters and return values.
- **Control Flow**: `if/elif/else` logic and `while` loops.
- **Arrays**: Built-in support for array literals and indexing.
- **Classes & Structs**: Object-oriented features with property access.
- **Bytecode VM**: Executes code on a fast, stack-based virtual machine.
- **Standard Library**: Built-in functions for output (`emit`) and math (`sqrt`, `pow`).

## Getting Started

### Installation

1. Clone the repository.
2. Build the project using Rust:
   ```bash
   cargo build --release
   ```
3. (Optional) Move the binary to your PATH to use the `quin` command globally.

### Usage

Create a file with the `.qn` extension (e.g., `hello.qn`):

```quin
let name = "World"
emit("Hello, " + name)
```

Run it using the interpreter:
```bash
quin hello.qn
```

## License

MIT License
