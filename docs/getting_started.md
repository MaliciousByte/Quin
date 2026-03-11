# Getting Started with Quin

This guide will help you install Quin and run your first program.

## Installation

### Prerequisites
- **Rust Toolchain**: Quin is built with Rust. If you don't have it, install it from [rust-lang.org](https://www.rust-lang.org/).

### Building from Source
1. Clone the repository:
   ```bash
   git clone https://github.com/MaliciousByte/Quin.git
   cd Quin
   ```
2. Build the project:
   ```bash
   cargo build --release
   ```
3. Add the binary to your path:
   The compiled binary will be located at `target/release/quin`. Add this directory to your system's `PATH`.

## Your First Program

Create a file named `hello.qn`:

```quin
emit("Hello, Quin!");
```

Run it using the Quin CLI:

```bash
quin hello.qn
```

## Running Tests
To verify your installation, you can run the comprehensive test suite:

```bash
quin comprehensive_test.qn
```
