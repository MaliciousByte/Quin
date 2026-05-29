// ─────────────────────────────────────────────────────────────────────────────
// Hotaru JIT Emitter — HCIR → x86-64 machine code
//
// Walks the Vec<HcirOp> produced by the lift pass and emits native x86-64
// instructions using the assembler. Handles forward-jump relocations,
// type guards with deopt fallback, and cross-platform calling convention.
// ─────────────────────────────────────────────────────────────────────────────

use crate::value::Function;
use crate::jit::hotaru::ir::hcir::{HcirOp, LiftResult};
use super::assembler::*;
use super::reloc::{HotaruReloc, patch_relocations};
use super::execmem;

/// Compile a function's HCIR into executable x86-64 machine code.
/// Returns a pointer to the native function, or null if compilation fails.
///
/// The generated function has signature:
///   extern "C" fn(vm: *mut VM, slots: *const Value) -> Value
pub fn compile_hotaru(function: &Function, lift: &LiftResult) -> *const u8 {
    let mut asm = Assembler::new();
    let mut relocs: Vec<HotaruReloc> = Vec::new();

    // Map from bytecode IP → code buffer offset (for jump target resolution)
    let mut bc_ip_to_code_offset: Vec<usize> = Vec::with_capacity(lift.ops.len() + 1);

    // Pointer to the constants array for LoadConst
    let constants_ptr = function.chunk.constants.as_ptr() as u64;

    // ── Prologue ─────────────────────────────────────────────────────────
    asm.emit_prologue();

    // Track epilogue location — will be set after all ops
    // We need a place to jump to for returns and deopts
    let mut epilogue_relocs: Vec<usize> = Vec::new();

    // ── Emit each HCIR op ────────────────────────────────────────────────
    for (hcir_idx, op) in lift.ops.iter().enumerate() {
        // Record code offset for this bytecode IP
        // Find the bytecode IP that maps to this HCIR index
        // Since bc_to_hcir[bc_ip] == hcir_idx, we need to record
        // at the right bc_ip. We track by HCIR index linearly.
        bc_ip_to_code_offset.push(asm.pos());

        match op {
            // ── Integer Arithmetic ──────────────────────────────────
            HcirOp::AddInt { dst, src1, src2 } => {
                emit_int_binop(&mut asm, *dst, *src1, *src2, IntBinOp::Add);
            }
            HcirOp::SubInt { dst, src1, src2 } => {
                emit_int_binop(&mut asm, *dst, *src1, *src2, IntBinOp::Sub);
            }
            HcirOp::MulInt { dst, src1, src2 } => {
                emit_int_binop(&mut asm, *dst, *src1, *src2, IntBinOp::Mul);
            }
            HcirOp::DivInt { dst, src1, src2 } => {
                emit_int_binop(&mut asm, *dst, *src1, *src2, IntBinOp::Div);
            }
            HcirOp::ModInt { dst, src1, src2 } => {
                emit_int_binop(&mut asm, *dst, *src1, *src2, IntBinOp::Mod);
            }
            HcirOp::NegateInt { dst, src } => {
                asm.unbox_int(*src, Reg64::RAX);
                asm.neg_r(Reg64::RAX);
                asm.rebox_int(Reg64::RAX, *dst);
            }

            // ── Generic Arithmetic (emit int path with deopt guard) ─
            HcirOp::Add { dst, src1, src2 } => {
                let deopt_patch = asm.emit_int_guard(*src1);
                let deopt_patch2 = asm.emit_int_guard(*src2);
                emit_int_binop(&mut asm, *dst, *src1, *src2, IntBinOp::Add);
                let skip = asm.jmp_rel32();
                // Deopt targets
                let deopt_pos = asm.pos();
                asm.patch_jmp(deopt_patch, deopt_pos);
                asm.patch_jmp(deopt_patch2, deopt_pos);
                let deopt_jmp = asm.emit_deopt(hcir_idx);
                epilogue_relocs.push(deopt_jmp);
                asm.patch_jmp(skip, asm.pos());
            }
            HcirOp::Sub { dst, src1, src2 } => {
                let deopt_patch = asm.emit_int_guard(*src1);
                let deopt_patch2 = asm.emit_int_guard(*src2);
                emit_int_binop(&mut asm, *dst, *src1, *src2, IntBinOp::Sub);
                let skip = asm.jmp_rel32();
                let deopt_pos = asm.pos();
                asm.patch_jmp(deopt_patch, deopt_pos);
                asm.patch_jmp(deopt_patch2, deopt_pos);
                let deopt_jmp = asm.emit_deopt(hcir_idx);
                epilogue_relocs.push(deopt_jmp);
                asm.patch_jmp(skip, asm.pos());
            }
            HcirOp::Mul { dst, src1, src2 } => {
                let deopt_patch = asm.emit_int_guard(*src1);
                let deopt_patch2 = asm.emit_int_guard(*src2);
                emit_int_binop(&mut asm, *dst, *src1, *src2, IntBinOp::Mul);
                let skip = asm.jmp_rel32();
                let deopt_pos = asm.pos();
                asm.patch_jmp(deopt_patch, deopt_pos);
                asm.patch_jmp(deopt_patch2, deopt_pos);
                let deopt_jmp = asm.emit_deopt(hcir_idx);
                epilogue_relocs.push(deopt_jmp);
                asm.patch_jmp(skip, asm.pos());
            }
            HcirOp::Div { dst, src1, src2 } => {
                let deopt_patch = asm.emit_int_guard(*src1);
                let deopt_patch2 = asm.emit_int_guard(*src2);
                emit_int_binop(&mut asm, *dst, *src1, *src2, IntBinOp::Div);
                let skip = asm.jmp_rel32();
                let deopt_pos = asm.pos();
                asm.patch_jmp(deopt_patch, deopt_pos);
                asm.patch_jmp(deopt_patch2, deopt_pos);
                let deopt_jmp = asm.emit_deopt(hcir_idx);
                epilogue_relocs.push(deopt_jmp);
                asm.patch_jmp(skip, asm.pos());
            }
            HcirOp::Negate { dst, src } => {
                let deopt_patch = asm.emit_int_guard(*src);
                asm.unbox_int(*src, Reg64::RAX);
                asm.neg_r(Reg64::RAX);
                asm.rebox_int(Reg64::RAX, *dst);
                let skip = asm.jmp_rel32();
                let deopt_pos = asm.pos();
                asm.patch_jmp(deopt_patch, deopt_pos);
                let deopt_jmp = asm.emit_deopt(hcir_idx);
                epilogue_relocs.push(deopt_jmp);
                asm.patch_jmp(skip, asm.pos());
            }

            // ── Float Arithmetic ────────────────────────────────────
            HcirOp::AddFloat { dst, src1, src2 } => {
                emit_float_binop(&mut asm, *dst, *src1, *src2, FloatBinOp::Add);
            }
            HcirOp::SubFloat { dst, src1, src2 } => {
                emit_float_binop(&mut asm, *dst, *src1, *src2, FloatBinOp::Sub);
            }
            HcirOp::MulFloat { dst, src1, src2 } => {
                emit_float_binop(&mut asm, *dst, *src1, *src2, FloatBinOp::Mul);
            }
            HcirOp::DivFloat { dst, src1, src2 } => {
                emit_float_binop(&mut asm, *dst, *src1, *src2, FloatBinOp::Div);
            }
            HcirOp::NegateFloat { dst, src } => {
                asm.unbox_float(*src, XmmReg::XMM0);
                asm.xorpd(XmmReg::XMM1, XmmReg::XMM1);
                asm.subsd(XmmReg::XMM1, XmmReg::XMM0);
                asm.rebox_float(XmmReg::XMM1, *dst);
            }

            // ── Integer Comparisons ─────────────────────────────────
            HcirOp::LtInt { dst, src1, src2 } => {
                emit_int_cmp(&mut asm, *dst, *src1, *src2, Cc::L);
            }
            HcirOp::GtInt { dst, src1, src2 } => {
                emit_int_cmp(&mut asm, *dst, *src1, *src2, Cc::NLE);
            }
            HcirOp::EqInt { dst, src1, src2 } => {
                emit_int_cmp(&mut asm, *dst, *src1, *src2, Cc::E);
            }
            HcirOp::NeqInt { dst, src1, src2 } => {
                emit_int_cmp(&mut asm, *dst, *src1, *src2, Cc::NE);
            }

            // ── Float Comparisons ───────────────────────────────────
            HcirOp::LtFloat { dst, src1, src2 } => {
                emit_float_cmp(&mut asm, *dst, *src1, *src2, Cc::B);
            }
            HcirOp::GtFloat { dst, src1, src2 } => {
                emit_float_cmp(&mut asm, *dst, *src1, *src2, Cc::NBE);
            }
            HcirOp::EqFloat { dst, src1, src2 } => {
                emit_float_cmp(&mut asm, *dst, *src1, *src2, Cc::E);
            }
            HcirOp::NeqFloat { dst, src1, src2 } => {
                emit_float_cmp(&mut asm, *dst, *src1, *src2, Cc::NE);
            }

            // ── Generic Comparisons (with type guard) ───────────────
            HcirOp::Less { dst, src1, src2 } => {
                let deopt_patch = asm.emit_int_guard(*src1);
                let deopt_patch2 = asm.emit_int_guard(*src2);
                emit_int_cmp(&mut asm, *dst, *src1, *src2, Cc::L);
                let skip = asm.jmp_rel32();
                let deopt_pos = asm.pos();
                asm.patch_jmp(deopt_patch, deopt_pos);
                asm.patch_jmp(deopt_patch2, deopt_pos);
                let deopt_jmp = asm.emit_deopt(hcir_idx);
                epilogue_relocs.push(deopt_jmp);
                asm.patch_jmp(skip, asm.pos());
            }
            HcirOp::Greater { dst, src1, src2 } => {
                let deopt_patch = asm.emit_int_guard(*src1);
                let deopt_patch2 = asm.emit_int_guard(*src2);
                emit_int_cmp(&mut asm, *dst, *src1, *src2, Cc::NLE);
                let skip = asm.jmp_rel32();
                let deopt_pos = asm.pos();
                asm.patch_jmp(deopt_patch, deopt_pos);
                asm.patch_jmp(deopt_patch2, deopt_pos);
                let deopt_jmp = asm.emit_deopt(hcir_idx);
                epilogue_relocs.push(deopt_jmp);
                asm.patch_jmp(skip, asm.pos());
            }
            HcirOp::Equal { dst, src1, src2 } => {
                let deopt_patch = asm.emit_int_guard(*src1);
                let deopt_patch2 = asm.emit_int_guard(*src2);
                emit_int_cmp(&mut asm, *dst, *src1, *src2, Cc::E);
                let skip = asm.jmp_rel32();
                let deopt_pos = asm.pos();
                asm.patch_jmp(deopt_patch, deopt_pos);
                asm.patch_jmp(deopt_patch2, deopt_pos);
                let deopt_jmp = asm.emit_deopt(hcir_idx);
                epilogue_relocs.push(deopt_jmp);
                asm.patch_jmp(skip, asm.pos());
            }
            HcirOp::Neq { dst, src1, src2 } => {
                let deopt_patch = asm.emit_int_guard(*src1);
                let deopt_patch2 = asm.emit_int_guard(*src2);
                emit_int_cmp(&mut asm, *dst, *src1, *src2, Cc::NE);
                let skip = asm.jmp_rel32();
                let deopt_pos = asm.pos();
                asm.patch_jmp(deopt_patch, deopt_pos);
                asm.patch_jmp(deopt_patch2, deopt_pos);
                let deopt_jmp = asm.emit_deopt(hcir_idx);
                epilogue_relocs.push(deopt_jmp);
                asm.patch_jmp(skip, asm.pos());
            }

            // ── Logic ───────────────────────────────────────────────
            HcirOp::Not { dst, src } => {
                // Test if value is falsey, then box the boolean result
                asm.emit_falsey_test(*src);
                // ZF=1 means falsey → Not should return true
                asm.setcc(Cc::E);
                asm.movzx_rax_al();
                asm.rebox_bool(Reg64::RAX, *dst);
            }

            // ── Loads ───────────────────────────────────────────────
            HcirOp::LoadConst { dst, const_idx } => {
                // Load constant value bits from the constants array
                // mov rax, constants_ptr
                asm.mov_ri64(Reg64::RAX, constants_ptr);
                // mov rax, [rax + const_idx * 8]
                asm.mov_rm(Reg64::RAX, Reg64::RAX, (*const_idx as i32) * 8);
                // Store to slot (raw bits copy — no refcount for JIT frame)
                asm.store_slot(*dst, Reg64::RAX);
            }
            HcirOp::GetGlobal { dst, const_idx } => {
                let get_global_ptr = crate::jit::libcalls::quin_get_global as *const u8 as u64;
                asm.mov_rr(ARG0, VM_REG);
                asm.mov_ri64(ARG1, constants_ptr);
                asm.mov_ri64(ARG2, *const_idx as u64);
                asm.mov_ri64(Reg64::RAX, get_global_ptr);
                asm.call_indirect_reg(Reg64::RAX);
                asm.store_slot(*dst, Reg64::RAX);
            }
            HcirOp::LoadNull { dst } => {
                asm.mov_ri64(Reg64::RAX, crate::value::QNAN | crate::value::TAG_NULL);
                asm.store_slot(*dst, Reg64::RAX);
            }
            HcirOp::LoadTrue { dst } => {
                asm.mov_ri64(Reg64::RAX, crate::value::QNAN | crate::value::TAG_TRUE);
                asm.store_slot(*dst, Reg64::RAX);
            }
            HcirOp::LoadFalse { dst } => {
                asm.mov_ri64(Reg64::RAX, crate::value::QNAN | crate::value::TAG_FALSE);
                asm.store_slot(*dst, Reg64::RAX);
            }
            HcirOp::Move { dst, src } => {
                asm.load_slot(Reg64::RAX, *src);
                asm.store_slot(*dst, Reg64::RAX);
            }

            // ── Control Flow ────────────────────────────────────────
            HcirOp::JumpIfFalse { src, offset } => {
                asm.emit_falsey_test(*src);
                // Jump if ZF=1 (falsey)
                let patch = asm.jcc_rel32(Cc::E);
                // Target is current_bc_ip + 1 + offset
                let target_bc_ip = hcir_idx + 1 + *offset as usize;
                relocs.push(HotaruReloc { patch_offset: patch, target_bc_ip });
            }
            HcirOp::JumpIfNull { src, offset } => {
                asm.load_slot(Reg64::RAX, *src);
                asm.mov_ri64(Reg64::R11, crate::value::QNAN | crate::value::TAG_NULL);
                asm.cmp_rr(Reg64::RAX, Reg64::R11);
                let patch = asm.jcc_rel32(Cc::E);
                let target_bc_ip = hcir_idx + 1 + *offset as usize;
                relocs.push(HotaruReloc { patch_offset: patch, target_bc_ip });
            }
            HcirOp::Jump { offset } => {
                let patch = asm.jmp_rel32();
                let target_bc_ip = hcir_idx + 1 + *offset as usize;
                relocs.push(HotaruReloc { patch_offset: patch, target_bc_ip });
            }
            HcirOp::Loop { offset } => {
                // Backward jump — target is current_bc_ip + 1 - offset
                let target_bc_ip = (hcir_idx + 1).wrapping_sub(*offset as usize);
                if target_bc_ip < bc_ip_to_code_offset.len() {
                    let target_offset = bc_ip_to_code_offset[target_bc_ip];
                    let rel = (target_offset as i64 - (asm.pos() as i64 + 5)) as i32;
                    asm.jmp_rel32_imm(rel);
                } else {
                    // Can't resolve — deopt
                    let deopt_jmp = asm.emit_deopt(hcir_idx);
                    epilogue_relocs.push(deopt_jmp);
                }
            }

            // ── Calls ───────────────────────────────────────────────
            HcirOp::CallOut { dst, callee_reg, arg_count } => {
                emit_call_out(&mut asm, *dst, *callee_reg, *arg_count);
            }
            HcirOp::CallIn { dst, callee_native_ptr, callee_reg, arg_count } => {
                emit_call_in(&mut asm, *dst, *callee_native_ptr, *callee_reg, *arg_count);
            }

            // ── Return ──────────────────────────────────────────────
            HcirOp::Return { src } => {
                let patch = asm.emit_return(*src);
                epilogue_relocs.push(patch);
            }

            // ── Deopt ───────────────────────────────────────────────
            HcirOp::Deopt { bc_ip } => {
                let patch = asm.emit_deopt(*bc_ip);
                epilogue_relocs.push(patch);
            }
        }
    }

    // Record final offset (for jumps past the end)
    bc_ip_to_code_offset.push(asm.pos());

    // ── Epilogue ─────────────────────────────────────────────────────────
    let epilogue_offset = asm.pos();
    asm.emit_epilogue();

    // ── Patch all epilogue jumps ─────────────────────────────────────────
    for patch in &epilogue_relocs {
        asm.patch_jmp(*patch, epilogue_offset);
    }

    // ── Patch forward-jump relocations ──────────────────────────────────
    patch_relocations(&mut asm.code, &relocs, &bc_ip_to_code_offset);

    // ── Allocate executable memory and copy ──────────────────────────────
    execmem::alloc_executable(&asm.code)
}

// ─── Helper: Integer binary operation ────────────────────────────────────────

enum IntBinOp { Add, Sub, Mul, Div, Mod }

fn emit_int_binop(asm: &mut Assembler, dst: u8, src1: u8, src2: u8, op: IntBinOp) {
    asm.unbox_int(src1, Reg64::RAX);
    asm.unbox_int(src2, Reg64::RCX);
    match op {
        IntBinOp::Add => asm.add_rr(Reg64::RAX, Reg64::RCX),
        IntBinOp::Sub => asm.sub_rr(Reg64::RAX, Reg64::RCX),
        IntBinOp::Mul => asm.imul_rr(Reg64::RAX, Reg64::RCX),
        IntBinOp::Div => {
            asm.cqo();
            asm.idiv_r(Reg64::RCX);
            // quotient in RAX
        }
        IntBinOp::Mod => {
            asm.cqo();
            asm.idiv_r(Reg64::RCX);
            // remainder in RDX — move to RAX
            asm.mov_rr(Reg64::RAX, Reg64::RDX);
        }
    }
    asm.rebox_int(Reg64::RAX, dst);
}

// ─── Helper: Float binary operation ─────────────────────────────────────────

enum FloatBinOp { Add, Sub, Mul, Div }

fn emit_float_binop(asm: &mut Assembler, dst: u8, src1: u8, src2: u8, op: FloatBinOp) {
    asm.unbox_float(src1, XmmReg::XMM0);
    asm.unbox_float(src2, XmmReg::XMM1);
    match op {
        FloatBinOp::Add => asm.addsd(XmmReg::XMM0, XmmReg::XMM1),
        FloatBinOp::Sub => asm.subsd(XmmReg::XMM0, XmmReg::XMM1),
        FloatBinOp::Mul => asm.mulsd(XmmReg::XMM0, XmmReg::XMM1),
        FloatBinOp::Div => asm.divsd(XmmReg::XMM0, XmmReg::XMM1),
    }
    asm.rebox_float(XmmReg::XMM0, dst);
}

// ─── Helper: Integer comparison ─────────────────────────────────────────────

fn emit_int_cmp(asm: &mut Assembler, dst: u8, src1: u8, src2: u8, cc: Cc) {
    asm.unbox_int(src1, Reg64::RAX);
    asm.unbox_int(src2, Reg64::RCX);
    asm.cmp_rr(Reg64::RAX, Reg64::RCX);
    asm.setcc(cc);
    asm.movzx_rax_al();
    asm.rebox_bool(Reg64::RAX, dst);
}

// ─── Helper: Float comparison ───────────────────────────────────────────────

fn emit_float_cmp(asm: &mut Assembler, dst: u8, src1: u8, src2: u8, cc: Cc) {
    asm.unbox_float(src1, XmmReg::XMM0);
    asm.unbox_float(src2, XmmReg::XMM1);
    asm.comisd(XmmReg::XMM0, XmmReg::XMM1);
    asm.setcc(cc);
    asm.movzx_rax_al();
    asm.rebox_bool(Reg64::RAX, dst);
}

// ─── Helper: CallOut (external call via quin_call_generic) ──────────────────

fn emit_call_out(asm: &mut Assembler, dst: u8, callee_reg: u8, arg_count: u8) {
    // Set up args for quin_call_generic:
    //   arg0 = VM* (from R13)
    //   arg1 = callee NaN-boxed bits (i64)
    //   arg2 = pointer to first arg in slot array
    //   arg3 = arg count
    let call_generic_ptr = crate::jit::libcalls::quin_call_generic as *const u8 as u64;

    asm.mov_rr(ARG0, VM_REG);
    asm.load_slot(ARG1, callee_reg);
    // Args start at callee_reg + 1
    asm.lea(ARG2, SLOTS_REG, (callee_reg as i32 + 1) * 8);
    asm.mov_ri64(ARG3, arg_count as u64);

    // Load function pointer and call
    asm.mov_ri64(Reg64::RAX, call_generic_ptr);
    asm.call_indirect_reg(Reg64::RAX);

    // Store result to dst slot
    asm.store_slot(dst, Reg64::RAX);
}

// ─── Helper: CallIn (direct call to known native pointer) ───────────────────

fn emit_call_in(asm: &mut Assembler, dst: u8, callee_native_ptr: usize, callee_reg: u8, arg_count: u8) {
    // For self-recursive calls, we call the native function directly.
    // The native function has signature: extern "C" fn(*mut VM, *const Value) -> Value
    //
    // We pass:
    //   arg0 = VM* (R13)
    //   arg1 = pointer to the callee slot (which is the new frame's slot base)
    //
    // The callee's arguments are in slots [callee_reg+1 .. callee_reg+1+arg_count]
    // But the native function expects slots starting at the callee slot.

    // First, try using CallOut as a safe fallback that handles all the VM bookkeeping
    // For now, use the generic call path — direct calls require matching the
    // interpreter's frame setup exactly, which we do via quin_call_generic
    let _ = callee_native_ptr; // Will be used in Phase 4 for true direct calls
    emit_call_out(asm, dst, callee_reg, arg_count);
}
