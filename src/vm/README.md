# VM Module (Quin Execution Engine)

This directory contains the bytecode execution engine for the Quin programming language.

## Architecture & Structure
The VM has been modularized from a single monolithic `vm.rs` file into logical domains:
- `mod.rs`: The core `VM` struct definitions, memory/GC root configuration, interpretation APIs, and global state.
- `exec.rs`: The main opcode dispatch loop (`execute_op`). This is the hottest path in the interpreter.
- `calls.rs`: Dispatching functions for runtime calls (`call_value`, `call_closure`), managing stack frames, and handling the JIT engine integration boundary.
- `modules.rs`: Resolution and loading logic for the module/import system.
- `upvalues.rs`: Management of closures and closed-over variables.
- `helpers.rs`: Stack manipulation utilities (push, pop, peek), binary mathematical operations, and exception handling logic.
