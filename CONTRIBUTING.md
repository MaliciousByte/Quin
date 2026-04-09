# Contributing to Quin

First off, thank you for considering contributing to Quin! Quin is in active development, and the codebase is intentionally designed to be clean, readable, and modular. Whether you are well-versed in Rust or just starting to look into virtual machines, compilers, or language design, there is meaningful work to do at every level.

## Getting Started

Before diving into code:
1. **Familiarize Yourself with the Architecture:** Read through `src/value`, `src/vm`, and `src/jit` to understand the flow from bytecode interpretation to JIT compilation. 
2. **Build the Project:** Use stable Rust to build the project locally `cargo build --release`.
3. **Run the Test Suite:** Ensure your environment is set up by passing all tests `.\tests\run_all.ps1`.
4. **Communicate Early:** Open an issue to discuss significant architectural changes or new features before writing the code to ensure efforts aren't duplicated.

## Good First Areas

If you want to contribute but aren't sure where to start, here are high-impact areas that are welcoming for new contributors:
- **Expanding JIT Opcode Coverage:** Implement missing handlers in `src/jit/codegen.rs`.
- **Standard Library Modules:** Add highly requested utility functions to modules inside `src/stdlib/`.
- **Testing:** Write edge-case triggering `.qn` tests to broaden the scope of the test suite. 
- **Documentation:** Help polish and expand the documentation found in the `docs/` directory.

## Contribution Guidelines

1. **Test Coverage:** Any new feature must come with associated `.qn` tests. 
2. **Zero Warnings Policy:** All contributions must pass the full test suite with *zero* compiler warnings on the release build (`cargo build --release`). Please also verify through `cargo clippy`.
3. **Naming and Formatting:** Utilize idiomatic Rust formatting (via `cargo fmt`). Keep the Quin parser/compiler code readable and direct.

We are excited to build the next generation of dynamic execution performance. Thank you for being a part of it!