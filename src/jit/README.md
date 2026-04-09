# JIT Module (Just-In-Time Compiler)

This directory implements the Cranelift-based JIT compiler to accelerate hot loops and recursive calls in Quin.

## Architecture
- `mod.rs`: The `JitEngine` structural definition, compiling heuristics, and the primary cache of compiled functions.
- `codegen.rs`: The IR emission engine. Discovers basic blocks, runs type inference, and translates Quin bytecodes into Cranelift IR (the largest component).
- `libcalls.rs`: Extern "C" bindings allowing the generated native code to cleanly interface back with the Rust VM internals (e.g. array indexing, global lookups).
- `types.rs`: JIT type inference heuristics (`JitType`). Essential for eliding NaN checks and directly emitting native float and integer operations when sound.
