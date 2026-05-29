// ─────────────────────────────────────────────────────────────────────────────
// HCIR Lift Pass — Bytecode → HcirOp translation
//
// Reads the register-based bytecode from a Function's chunk and produces
// a Vec<HcirOp> plus a bytecode-IP-to-HCIR-index mapping for jump resolution.
// ─────────────────────────────────────────────────────────────────────────────

use crate::value::Function;
use crate::frontend::chunk::{
    decode_inst, decode_inst_imm16,
    OP_LOAD_CONST, OP_LOAD_NULL, OP_LOAD_TRUE, OP_LOAD_FALSE, OP_MOVE,
    OP_GET_GLOBAL,
    OP_ADD, OP_SUBTRACT, OP_MULTIPLY, OP_DIVIDE, OP_NEGATE, OP_NOT,
    OP_ADD_INT, OP_SUB_INT, OP_MUL_INT, OP_DIV_INT,
    OP_ADD_FLOAT, OP_SUB_FLOAT, OP_MUL_FLOAT, OP_DIV_FLOAT,
    OP_LESS, OP_GREATER, OP_EQUAL, OP_NEQ,
    OP_LT_INT, OP_GT_INT, OP_EQ_INT, OP_NEQ_INT,
    OP_LT_FLOAT, OP_GT_FLOAT, OP_EQ_FLOAT, OP_NEQ_FLOAT,
    OP_JUMP_IF_FALSE, OP_JUMP, OP_LOOP, OP_JUMP_IF_NULL,
    OP_CALL, OP_RETURN,
};
use super::hcir::{HcirOp, LiftResult};

/// Lift a function's bytecode into HCIR.
///
/// Each bytecode instruction maps to exactly one HcirOp. Unsupported opcodes
/// produce `HcirOp::Deopt { bc_ip }` so the JIT can fall back to the
/// interpreter for those paths.
pub fn lift_function(function: &Function) -> LiftResult {
    let code = &function.chunk.code;
    let mut ops = Vec::with_capacity(code.len());
    let mut bc_to_hcir: Vec<usize> = Vec::with_capacity(code.len());

    // Check if this function has a known native pointer (for self-recursion detection)
    let self_native_ptr = function.native_ptr.load(std::sync::atomic::Ordering::Relaxed) as usize;

    for (bc_ip, &inst) in code.iter().enumerate() {
        bc_to_hcir.push(ops.len());

        // Try 4-operand decode first
        let (op, a, b, c) = decode_inst(inst);

        let hcir_op = match op {
            // ── Loads ────────────────────────────────────────────────────
            OP_LOAD_CONST => {
                let (_, dst, const_idx) = decode_inst_imm16(inst);
                HcirOp::LoadConst { dst, const_idx }
            }
            OP_GET_GLOBAL => {
                let (_, dst, const_idx) = decode_inst_imm16(inst);
                HcirOp::GetGlobal { dst, const_idx }
            }
            OP_LOAD_NULL => HcirOp::LoadNull { dst: a },
            OP_LOAD_TRUE => HcirOp::LoadTrue { dst: a },
            OP_LOAD_FALSE => HcirOp::LoadFalse { dst: a },
            OP_MOVE => HcirOp::Move { dst: a, src: b },

            // ── Generic arithmetic ──────────────────────────────────────
            OP_ADD => HcirOp::Add { dst: a, src1: b, src2: c },
            OP_SUBTRACT => HcirOp::Sub { dst: a, src1: b, src2: c },
            OP_MULTIPLY => HcirOp::Mul { dst: a, src1: b, src2: c },
            OP_DIVIDE => HcirOp::Div { dst: a, src1: b, src2: c },
            OP_NEGATE => HcirOp::Negate { dst: a, src: b },

            // ── Integer-specialized arithmetic ──────────────────────────
            OP_ADD_INT => HcirOp::AddInt { dst: a, src1: b, src2: c },
            OP_SUB_INT => HcirOp::SubInt { dst: a, src1: b, src2: c },
            OP_MUL_INT => HcirOp::MulInt { dst: a, src1: b, src2: c },
            OP_DIV_INT => HcirOp::DivInt { dst: a, src1: b, src2: c },

            // ── Float-specialized arithmetic ────────────────────────────
            OP_ADD_FLOAT => HcirOp::AddFloat { dst: a, src1: b, src2: c },
            OP_SUB_FLOAT => HcirOp::SubFloat { dst: a, src1: b, src2: c },
            OP_MUL_FLOAT => HcirOp::MulFloat { dst: a, src1: b, src2: c },
            OP_DIV_FLOAT => HcirOp::DivFloat { dst: a, src1: b, src2: c },

            // ── Logic ───────────────────────────────────────────────────
            OP_NOT => HcirOp::Not { dst: a, src: b },

            // ── Generic comparisons ─────────────────────────────────────
            OP_LESS => HcirOp::Less { dst: a, src1: b, src2: c },
            OP_GREATER => HcirOp::Greater { dst: a, src1: b, src2: c },
            OP_EQUAL => HcirOp::Equal { dst: a, src1: b, src2: c },
            OP_NEQ => HcirOp::Neq { dst: a, src1: b, src2: c },

            // ── Integer-specialized comparisons ─────────────────────────
            OP_LT_INT => HcirOp::LtInt { dst: a, src1: b, src2: c },
            OP_GT_INT => HcirOp::GtInt { dst: a, src1: b, src2: c },
            OP_EQ_INT => HcirOp::EqInt { dst: a, src1: b, src2: c },
            OP_NEQ_INT => HcirOp::NeqInt { dst: a, src1: b, src2: c },

            // ── Float-specialized comparisons ───────────────────────────
            OP_LT_FLOAT => HcirOp::LtFloat { dst: a, src1: b, src2: c },
            OP_GT_FLOAT => HcirOp::GtFloat { dst: a, src1: b, src2: c },
            OP_EQ_FLOAT => HcirOp::EqFloat { dst: a, src1: b, src2: c },
            OP_NEQ_FLOAT => HcirOp::NeqFloat { dst: a, src1: b, src2: c },

            // ── Control flow ────────────────────────────────────────────
            OP_JUMP_IF_FALSE => {
                let (_, src, offset) = decode_inst_imm16(inst);
                HcirOp::JumpIfFalse { src, offset }
            }
            OP_JUMP_IF_NULL => {
                let (_, src, offset) = decode_inst_imm16(inst);
                HcirOp::JumpIfNull { src, offset }
            }
            OP_JUMP => {
                let (_, _, offset) = decode_inst_imm16(inst);
                HcirOp::Jump { offset }
            }
            OP_LOOP => {
                let (_, _, offset) = decode_inst_imm16(inst);
                HcirOp::Loop { offset }
            }

            // ── Calls ───────────────────────────────────────────────────
            OP_CALL => {
                // dst=a, callee_reg=b, arg_count=c
                // Check for self-recursion: if the callee register will hold
                // the same function and we have a native pointer, emit CallIn
                if self_native_ptr != 0 {
                    HcirOp::CallIn {
                        dst: a,
                        callee_native_ptr: self_native_ptr,
                        callee_reg: b,
                        arg_count: c,
                    }
                } else {
                    HcirOp::CallOut { dst: a, callee_reg: b, arg_count: c }
                }
            }

            // ── Return ──────────────────────────────────────────────────
            OP_RETURN => HcirOp::Return { src: a },

            // ── Everything else → deopt ─────────────────────────────────
            _ => HcirOp::Deopt { bc_ip },
        };

        ops.push(hcir_op);
    }

    LiftResult { ops, bc_to_hcir }
}
