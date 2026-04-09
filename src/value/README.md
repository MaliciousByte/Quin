# Value Module (Runtime Objects)

This directory defines the types representing values moving through the Quin execution stack.

## Architecture
Using a NaN-boxing architecture (64-bit IEEE 754), Quin stores primitives without allocation overhead. Pointers to heap objects steal the sign bit to represent managed references.
- `mod.rs`: The `Value` struct (the NaN-box itself), bit-twiddling logic for encoding ints/floats/bools/pointers, and core memory management (`mark`/`unmark` for atomic refcounting).
- `function.rs`: All function-related runtime values: user `Closure` and `Function` structures, `NativeFn`, and upvalue management tools.
- `types.rs`: Structs governing user-defined classes and structures: `ClassValue`, `InstanceValue`, and property definitions (`Shape`).
