// ─────────────────────────────────────────────────────────────────────────────
// HCIR — Hotaru Compact Intermediate Representation
//
// A register-based IR that sits between Quin bytecode and x86-64 machine code.
// The lift pass (lift.rs) translates bytecode -> Vec<HcirOp>.
// The emitter (backend/emit.rs) translates Vec<HcirOp> -> x86-64.
// ─────────────────────────────────────────────────────────────────────────────

/// A single HCIR operation. Each variant maps closely to one bytecode
/// instruction but carries enough structure for the backend to emit
/// optimal machine code without re-decoding.
#[derive(Debug, Clone)]
pub enum HcirOp {
    // ── Arithmetic (integer-specialized) ─────────────────────────────────
    AddInt { dst: u8, src1: u8, src2: u8 },
    SubInt { dst: u8, src1: u8, src2: u8 },
    MulInt { dst: u8, src1: u8, src2: u8 },
    DivInt { dst: u8, src1: u8, src2: u8 },
    ModInt { dst: u8, src1: u8, src2: u8 },
    NegateInt { dst: u8, src: u8 },

    // ── Arithmetic (float-specialized) ───────────────────────────────────
    AddFloat { dst: u8, src1: u8, src2: u8 },
    SubFloat { dst: u8, src1: u8, src2: u8 },
    MulFloat { dst: u8, src1: u8, src2: u8 },
    DivFloat { dst: u8, src1: u8, src2: u8 },
    NegateFloat { dst: u8, src: u8 },

    // ── Arithmetic (generic / unspecialized) ──────────────────────────────
    Add { dst: u8, src1: u8, src2: u8 },
    Sub { dst: u8, src1: u8, src2: u8 },
    Mul { dst: u8, src1: u8, src2: u8 },
    Div { dst: u8, src1: u8, src2: u8 },
    Negate { dst: u8, src: u8 },

    // ── Comparisons (integer-specialized) ────────────────────────────────
    LtInt { dst: u8, src1: u8, src2: u8 },
    GtInt { dst: u8, src1: u8, src2: u8 },
    EqInt { dst: u8, src1: u8, src2: u8 },
    NeqInt { dst: u8, src1: u8, src2: u8 },

    // ── Comparisons (float-specialized) ──────────────────────────────────
    LtFloat { dst: u8, src1: u8, src2: u8 },
    GtFloat { dst: u8, src1: u8, src2: u8 },
    EqFloat { dst: u8, src1: u8, src2: u8 },
    NeqFloat { dst: u8, src1: u8, src2: u8 },

    // ── Comparisons (generic) ────────────────────────────────────────────
    Less { dst: u8, src1: u8, src2: u8 },
    Greater { dst: u8, src1: u8, src2: u8 },
    Equal { dst: u8, src1: u8, src2: u8 },
    Neq { dst: u8, src1: u8, src2: u8 },

    // ── Logic ────────────────────────────────────────────────────────────
    Not { dst: u8, src: u8 },

    // ── Loads ────────────────────────────────────────────────────────────
    LoadConst { dst: u8, const_idx: u16 },
    GetGlobal { dst: u8, const_idx: u16 },
    LoadNull { dst: u8 },
    LoadTrue { dst: u8 },
    LoadFalse { dst: u8 },
    Move { dst: u8, src: u8 },

    // ── Control flow ────────────────────────────────────────────────────
    /// Jump forward by `offset` bytecode instructions if slot `src` is falsey.
    JumpIfFalse { src: u8, offset: u16 },
    /// Jump forward by `offset` bytecode instructions if slot `src` is null.
    JumpIfNull { src: u8, offset: u16 },
    /// Unconditional forward jump by `offset` bytecode instructions.
    Jump { offset: u16 },
    /// Backward jump (loop) by `offset` bytecode instructions.
    Loop { offset: u16 },

    // ── Calls ───────────────────────────────────────────────────────────
    /// External call through the generic libcall trampoline.
    CallOut { dst: u8, callee_reg: u8, arg_count: u8 },
    /// Direct call to a known native function pointer (e.g. self-recursion).
    CallIn { dst: u8, callee_native_ptr: usize, callee_reg: u8, arg_count: u8 },

    // ── Return ──────────────────────────────────────────────────────────
    Return { src: u8 },

    // ── Deopt fallback ──────────────────────────────────────────────────
    /// Unsupported opcode — emit code to return Value::deopt(bc_ip)
    /// so the interpreter resumes at this bytecode IP.
    Deopt { bc_ip: usize },
}

/// Result of the lift pass.
pub struct LiftResult {
    /// The HCIR ops produced from the bytecode.
    pub ops: Vec<HcirOp>,
    /// Mapping from bytecode IP → index into `ops`.
    /// Used to resolve jump targets during emission.
    pub bc_to_hcir: Vec<usize>,
}
