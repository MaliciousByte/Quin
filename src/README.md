# Quin Language Source (`src/`)

This directory contains the entire source code for the dynamically-typed, JIT-accelerated bytecode interpreted Quin programming language.

## Architecture Overview
- **CLI & REPL**: `main.rs` processes arguments and entry operations while `repl.rs` handles the interactive runtime environment.
- **Frontend Compiler**:
  - `lexer.rs` & `token.rs`: Transforms character streams to structural lexical tokens.
  - `parser.rs` & `ast.rs`: Pratt Parser generating the AST.
  - `compiler.rs` & `chunk.rs`: Walker parsing the AST into 1D bytecode sequences (`Chunk` chunks).
- **Execution & Runtime**:
  - `vm/`: The virtual execution system powering the runtime.
  - `jit/`: Cranelift IR translation optimizing "hot path" branches into native instructions.
  - `value/`: NaN-boxed payload format and runtime heap memory implementations.
  - `stdlib/`: Native libraries bridged into the scope.
- **Memory**: `interner.rs` maintains the globally pooled strings. `obj.rs` is the generic heap wrapper.
