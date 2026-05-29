// ─────────────────────────────────────────────────────────────────────────────
// x86-64 Machine Code Assembler for Hotaru JIT
//
// Hand-emits raw x86-64 bytes into a Vec<u8> buffer. Supports:
// - Integer GPR operations (add, sub, imul, idiv, cmp, neg, not, etc.)
// - Memory load/store with base+displacement addressing
// - 64-bit immediate moves
// - Conditional and unconditional jumps (rel32)
// - SSE2 scalar double operations (addsd, subsd, mulsd, divsd, comisd)
// - NaN-box helper routines (unbox_int, rebox_int, unbox_float, rebox_float)
// - Cross-platform prologue/epilogue (Windows x64 / System V AMD64)
// ─────────────────────────────────────────────────────────────────────────────

use crate::value::{QNAN, TAG_INT, TAG_NULL, TAG_TRUE, TAG_FALSE, TAG_DEOPT, SIGN_BIT};

// ─── Register encoding ──────────────────────────────────────────────────────

/// 64-bit general-purpose register indices (matching x86-64 encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Reg64 {
    RAX = 0, RCX = 1, RDX = 2, RBX = 3,
    RSP = 4, RBP = 5, RSI = 6, RDI = 7,
    R8 = 8, R9 = 9, R10 = 10, R11 = 11,
    R12 = 12, R13 = 13, R14 = 14, R15 = 15,
}

/// XMM register indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum XmmReg {
    XMM0 = 0, XMM1 = 1, XMM2 = 2, XMM3 = 3,
    XMM4 = 4, XMM5 = 5, XMM6 = 6, XMM7 = 7,
}

/// Condition codes for Jcc instructions.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Cc {
    O = 0x0, NO = 0x1, B = 0x2, NB = 0x3,
    E = 0x4, NE = 0x5, BE = 0x6, NBE = 0x7,
    S = 0x8, NS = 0x9, P = 0xA, NP = 0xB,
    L = 0xC, NL = 0xD, LE = 0xE, NLE = 0xF,
}

// ─── Platform-specific argument registers ────────────────────────────────────

/// First argument register (VM*)
#[cfg(target_os = "windows")]
pub const ARG0: Reg64 = Reg64::RCX;
#[cfg(not(target_os = "windows"))]
pub const ARG0: Reg64 = Reg64::RDI;

/// Second argument register (Value* slots)
#[cfg(target_os = "windows")]
pub const ARG1: Reg64 = Reg64::RDX;
#[cfg(not(target_os = "windows"))]
pub const ARG1: Reg64 = Reg64::RSI;

/// Third argument register
#[cfg(target_os = "windows")]
pub const ARG2: Reg64 = Reg64::R8;
#[cfg(not(target_os = "windows"))]
pub const ARG2: Reg64 = Reg64::RDX;

/// Fourth argument register
#[cfg(target_os = "windows")]
pub const ARG3: Reg64 = Reg64::R9;
#[cfg(not(target_os = "windows"))]
pub const ARG3: Reg64 = Reg64::RCX;

/// VM pointer — stored in callee-saved R13
pub const VM_REG: Reg64 = Reg64::R13;
/// Slot array base — stored in callee-saved R14
pub const SLOTS_REG: Reg64 = Reg64::R14;

// ─── Assembler ──────────────────────────────────────────────────────────────

pub struct Assembler {
    pub code: Vec<u8>,
}

impl Assembler {
    pub fn new() -> Self {
        Assembler { code: Vec::with_capacity(4096) }
    }

    #[inline]
    pub fn pos(&self) -> usize {
        self.code.len()
    }

    // ─── Encoding helpers ───────────────────────────────────────────────

    #[inline]
    fn needs_rex(r: Reg64) -> bool {
        (r as u8) >= 8
    }

    /// REX prefix for instructions using two 64-bit registers.
    /// W=1 (64-bit), R = reg>>3, B = rm>>3
    #[inline]
    fn rex_rr(reg: Reg64, rm: Reg64) -> u8 {
        0x48 | ((reg as u8 >> 3) << 2) | (rm as u8 >> 3)
    }

    /// REX prefix for single-register ops (rm only, no reg field).
    #[inline]
    fn rex_b(rm: Reg64) -> u8 {
        0x48 | (rm as u8 >> 3)
    }

    /// ModRM byte: mod=11 (register), reg, rm
    #[inline]
    fn modrm_rr(reg: u8, rm: u8) -> u8 {
        0xC0 | ((reg & 7) << 3) | (rm & 7)
    }

    /// ModRM byte: mod=10 (disp32), reg, rm
    #[inline]
    fn modrm_disp32(reg: u8, base: u8) -> u8 {
        0x80 | ((reg & 7) << 3) | (base & 7)
    }

    /// ModRM byte: mod=00 (no displacement), reg, rm
    #[inline]
    fn modrm_indirect(reg: u8, base: u8) -> u8 {
        ((reg & 7) << 3) | (base & 7)
    }

    #[inline]
    fn emit(&mut self, byte: u8) {
        self.code.push(byte);
    }

    #[inline]
    fn emit_bytes(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }

    #[inline]
    fn emit_i32(&mut self, val: i32) {
        self.code.extend_from_slice(&val.to_le_bytes());
    }

    #[inline]
    fn emit_u64(&mut self, val: u64) {
        self.code.extend_from_slice(&val.to_le_bytes());
    }

    // ─── Prologue / Epilogue ────────────────────────────────────────────

    /// Emit function prologue: save callee-saved registers, set up VM_REG and SLOTS_REG.
    pub fn emit_prologue(&mut self) {
        // push rbp
        self.emit(0x55);
        // mov rbp, rsp
        self.mov_rr(Reg64::RBP, Reg64::RSP);
        // push r13
        self.push_r(Reg64::R13);
        // push r14
        self.push_r(Reg64::R14);
        // push rbx
        self.push_r(Reg64::RBX);
        // push r15 (extra callee-saved for scratch)
        self.push_r(Reg64::R15);

        // Align stack to 16 bytes (6 pushes = 48 bytes, already aligned on entry)
        // sub rsp, 32 — shadow space for Windows x64 ABI calls
        #[cfg(target_os = "windows")]
        {
            self.sub_ri32(Reg64::RSP, 32);
        }

        // mov r13, ARG0 (VM*)
        self.mov_rr(VM_REG, ARG0);
        // mov r14, ARG1 (Value* slots)
        self.mov_rr(SLOTS_REG, ARG1);
    }

    /// Emit function epilogue: restore callee-saved registers and return.
    /// The return value should already be in RAX.
    pub fn emit_epilogue(&mut self) {
        #[cfg(target_os = "windows")]
        {
            self.add_ri32(Reg64::RSP, 32);
        }

        self.pop_r(Reg64::R15);
        self.pop_r(Reg64::RBX);
        self.pop_r(Reg64::R14);
        self.pop_r(Reg64::R13);
        // pop rbp
        self.emit(0x5D);
        // ret
        self.emit(0xC3);
    }

    // ─── Push / Pop ─────────────────────────────────────────────────────

    pub fn push_r(&mut self, r: Reg64) {
        if Self::needs_rex(r) {
            self.emit(0x41); // REX.B
        }
        self.emit(0x50 + (r as u8 & 7));
    }

    pub fn pop_r(&mut self, r: Reg64) {
        if Self::needs_rex(r) {
            self.emit(0x41); // REX.B
        }
        self.emit(0x58 + (r as u8 & 7));
    }

    // ─── MOV ────────────────────────────────────────────────────────────

    /// mov dst, src (64-bit register to register)
    pub fn mov_rr(&mut self, dst: Reg64, src: Reg64) {
        self.emit(Self::rex_rr(src, dst));
        self.emit(0x89);
        self.emit(Self::modrm_rr(src as u8, dst as u8));
    }

    /// mov dst, imm64 (movabs)
    pub fn mov_ri64(&mut self, dst: Reg64, imm: u64) {
        self.emit(Self::rex_b(dst));
        self.emit(0xB8 + (dst as u8 & 7));
        self.emit_u64(imm);
    }

    /// mov dst, imm32 (sign-extended to 64-bit): mov dst, imm32
    pub fn mov_ri32(&mut self, dst: Reg64, imm: i32) {
        self.emit(Self::rex_b(dst));
        self.emit(0xC7);
        self.emit(Self::modrm_rr(0, dst as u8));
        self.emit_i32(imm);
    }

    /// mov dst, [base + disp32] (64-bit load from memory)
    pub fn mov_rm(&mut self, dst: Reg64, base: Reg64, disp: i32) {
        self.emit(Self::rex_rr(dst, base));
        self.emit(0x8B);
        // Special case: RSP/R12 as base needs SIB byte
        if (base as u8 & 7) == 4 {
            self.emit(Self::modrm_disp32(dst as u8, 4));
            self.emit(0x24); // SIB: scale=0, index=RSP(none), base=RSP
        } else {
            self.emit(Self::modrm_disp32(dst as u8, base as u8));
        }
        self.emit_i32(disp);
    }

    /// mov [base + disp32], src (64-bit store to memory)
    pub fn mov_mr(&mut self, base: Reg64, disp: i32, src: Reg64) {
        self.emit(Self::rex_rr(src, base));
        self.emit(0x89);
        if (base as u8 & 7) == 4 {
            self.emit(Self::modrm_disp32(src as u8, 4));
            self.emit(0x24);
        } else {
            self.emit(Self::modrm_disp32(src as u8, base as u8));
        }
        self.emit_i32(disp);
    }

    // ─── ALU (register-register, 64-bit) ────────────────────────────────

    /// add dst, src
    pub fn add_rr(&mut self, dst: Reg64, src: Reg64) {
        self.emit(Self::rex_rr(src, dst));
        self.emit(0x01);
        self.emit(Self::modrm_rr(src as u8, dst as u8));
    }

    /// sub dst, src
    pub fn sub_rr(&mut self, dst: Reg64, src: Reg64) {
        self.emit(Self::rex_rr(src, dst));
        self.emit(0x29);
        self.emit(Self::modrm_rr(src as u8, dst as u8));
    }

    /// imul dst, src (signed multiply, result in dst)
    pub fn imul_rr(&mut self, dst: Reg64, src: Reg64) {
        self.emit(Self::rex_rr(dst, src));
        self.emit(0x0F);
        self.emit(0xAF);
        self.emit(Self::modrm_rr(dst as u8, src as u8));
    }

    /// cqo — sign-extend RAX into RDX:RAX (before idiv)
    pub fn cqo(&mut self) {
        self.emit(0x48);
        self.emit(0x99);
    }

    /// idiv rm — signed divide RDX:RAX by rm, quotient in RAX, remainder in RDX
    pub fn idiv_r(&mut self, rm: Reg64) {
        self.emit(Self::rex_b(rm));
        self.emit(0xF7);
        self.emit(Self::modrm_rr(7, rm as u8));
    }

    /// cmp r1, r2
    pub fn cmp_rr(&mut self, r1: Reg64, r2: Reg64) {
        self.emit(Self::rex_rr(r2, r1));
        self.emit(0x39);
        self.emit(Self::modrm_rr(r2 as u8, r1 as u8));
    }

    /// test r1, r2
    pub fn test_rr(&mut self, r1: Reg64, r2: Reg64) {
        self.emit(Self::rex_rr(r2, r1));
        self.emit(0x85);
        self.emit(Self::modrm_rr(r2 as u8, r1 as u8));
    }

    /// neg rm (two's complement negate)
    pub fn neg_r(&mut self, rm: Reg64) {
        self.emit(Self::rex_b(rm));
        self.emit(0xF7);
        self.emit(Self::modrm_rr(3, rm as u8));
    }

    /// add dst, imm32
    pub fn add_ri32(&mut self, dst: Reg64, imm: i32) {
        self.emit(Self::rex_b(dst));
        self.emit(0x81);
        self.emit(Self::modrm_rr(0, dst as u8));
        self.emit_i32(imm);
    }

    /// sub dst, imm32
    pub fn sub_ri32(&mut self, dst: Reg64, imm: i32) {
        self.emit(Self::rex_b(dst));
        self.emit(0x81);
        self.emit(Self::modrm_rr(5, dst as u8));
        self.emit_i32(imm);
    }

    /// and dst, imm32 (sign-extended)
    pub fn and_ri32(&mut self, dst: Reg64, imm: i32) {
        self.emit(Self::rex_b(dst));
        self.emit(0x81);
        self.emit(Self::modrm_rr(4, dst as u8));
        self.emit_i32(imm);
    }

    /// or dst, src
    pub fn or_rr(&mut self, dst: Reg64, src: Reg64) {
        self.emit(Self::rex_rr(src, dst));
        self.emit(0x09);
        self.emit(Self::modrm_rr(src as u8, dst as u8));
    }

    /// and dst, src
    pub fn and_rr(&mut self, dst: Reg64, src: Reg64) {
        self.emit(Self::rex_rr(src, dst));
        self.emit(0x21);
        self.emit(Self::modrm_rr(src as u8, dst as u8));
    }

    /// shr dst, imm8
    pub fn shr_ri8(&mut self, dst: Reg64, imm: u8) {
        self.emit(Self::rex_b(dst));
        self.emit(0xC1);
        self.emit(Self::modrm_rr(5, dst as u8));
        self.emit(imm);
    }

    /// shl dst, imm8
    pub fn shl_ri8(&mut self, dst: Reg64, imm: u8) {
        self.emit(Self::rex_b(dst));
        self.emit(0xC1);
        self.emit(Self::modrm_rr(4, dst as u8));
        self.emit(imm);
    }

    // ─── SETcc ──────────────────────────────────────────────────────────

    /// setcc al (set byte based on condition)
    pub fn setcc(&mut self, cc: Cc) {
        self.emit(0x0F);
        self.emit(0x90 + cc as u8);
        self.emit(0xC0); // ModRM: al
    }

    /// movzx rax, al (zero-extend byte to 64-bit)
    pub fn movzx_rax_al(&mut self) {
        self.emit(0x48);
        self.emit(0x0F);
        self.emit(0xB6);
        self.emit(0xC0);
    }

    // ─── Jumps & Calls ──────────────────────────────────────────────────

    /// jmp rel32 — returns offset of the 4-byte rel32 field for patching.
    pub fn jmp_rel32(&mut self) -> usize {
        self.emit(0xE9);
        let patch = self.pos();
        self.emit_i32(0); // placeholder
        patch
    }

    /// jmp rel32 with known offset
    pub fn jmp_rel32_imm(&mut self, rel: i32) {
        self.emit(0xE9);
        self.emit_i32(rel);
    }

    /// jcc rel32 — conditional jump, returns offset of the 4-byte rel32 for patching.
    pub fn jcc_rel32(&mut self, cc: Cc) -> usize {
        self.emit(0x0F);
        self.emit(0x80 + cc as u8);
        let patch = self.pos();
        self.emit_i32(0); // placeholder
        patch
    }

    /// call reg (indirect)
    pub fn call_indirect_reg(&mut self, reg: Reg64) {
        if Self::needs_rex(reg) {
            self.emit(0x41);
        }
        self.emit(0xFF);
        self.emit(Self::modrm_rr(2, reg as u8));
    }

    /// lea dst, [base + disp32]
    pub fn lea(&mut self, dst: Reg64, base: Reg64, disp: i32) {
        self.emit(Self::rex_rr(dst, base));
        self.emit(0x8D);
        if (base as u8 & 7) == 4 {
            self.emit(Self::modrm_disp32(dst as u8, 4));
            self.emit(0x24);
        } else {
            self.emit(Self::modrm_disp32(dst as u8, base as u8));
        }
        self.emit_i32(disp);
    }

    // ─── SSE2 Scalar Double ─────────────────────────────────────────────

    /// movsd xmm, [base + disp32]
    pub fn movsd_load(&mut self, dst: XmmReg, base: Reg64, disp: i32) {
        self.emit(0xF2);
        // REX prefix if base is extended register
        if Self::needs_rex(base) || (dst as u8) >= 8 {
            self.emit(0x40 | ((dst as u8 >> 3) << 2) | (base as u8 >> 3));
        }
        self.emit(0x0F);
        self.emit(0x10);
        if (base as u8 & 7) == 4 {
            self.emit(Self::modrm_disp32(dst as u8, 4));
            self.emit(0x24);
        } else {
            self.emit(Self::modrm_disp32(dst as u8, base as u8));
        }
        self.emit_i32(disp);
    }

    /// movsd [base + disp32], xmm
    pub fn movsd_store(&mut self, base: Reg64, disp: i32, src: XmmReg) {
        self.emit(0xF2);
        if Self::needs_rex(base) || (src as u8) >= 8 {
            self.emit(0x40 | ((src as u8 >> 3) << 2) | (base as u8 >> 3));
        }
        self.emit(0x0F);
        self.emit(0x11);
        if (base as u8 & 7) == 4 {
            self.emit(Self::modrm_disp32(src as u8, 4));
            self.emit(0x24);
        } else {
            self.emit(Self::modrm_disp32(src as u8, base as u8));
        }
        self.emit_i32(disp);
    }

    /// addsd dst, src
    pub fn addsd(&mut self, dst: XmmReg, src: XmmReg) {
        self.emit(0xF2);
        self.emit(0x0F);
        self.emit(0x58);
        self.emit(Self::modrm_rr(dst as u8, src as u8));
    }

    /// subsd dst, src
    pub fn subsd(&mut self, dst: XmmReg, src: XmmReg) {
        self.emit(0xF2);
        self.emit(0x0F);
        self.emit(0x5C);
        self.emit(Self::modrm_rr(dst as u8, src as u8));
    }

    /// mulsd dst, src
    pub fn mulsd(&mut self, dst: XmmReg, src: XmmReg) {
        self.emit(0xF2);
        self.emit(0x0F);
        self.emit(0x59);
        self.emit(Self::modrm_rr(dst as u8, src as u8));
    }

    /// divsd dst, src
    pub fn divsd(&mut self, dst: XmmReg, src: XmmReg) {
        self.emit(0xF2);
        self.emit(0x0F);
        self.emit(0x5E);
        self.emit(Self::modrm_rr(dst as u8, src as u8));
    }

    /// comisd xmm1, xmm2 (sets EFLAGS for float comparison)
    pub fn comisd(&mut self, xmm1: XmmReg, xmm2: XmmReg) {
        self.emit(0x66);
        self.emit(0x0F);
        self.emit(0x2F);
        self.emit(Self::modrm_rr(xmm1 as u8, xmm2 as u8));
    }

    /// movq xmm, r64 (move GPR to XMM)
    pub fn movq_xmm_r64(&mut self, xmm: XmmReg, r: Reg64) {
        self.emit(0x66);
        self.emit(0x48 | ((xmm as u8 >> 3) << 2) | (r as u8 >> 3));
        self.emit(0x0F);
        self.emit(0x6E);
        self.emit(Self::modrm_rr(xmm as u8, r as u8));
    }

    /// movq r64, xmm (move XMM to GPR)
    pub fn movq_r64_xmm(&mut self, r: Reg64, xmm: XmmReg) {
        self.emit(0x66);
        self.emit(0x48 | ((xmm as u8 >> 3) << 2) | (r as u8 >> 3));
        self.emit(0x0F);
        self.emit(0x7E);
        self.emit(Self::modrm_rr(xmm as u8, r as u8));
    }

    /// xorpd dst, src (zero out an XMM register: xorpd xmm, xmm)
    pub fn xorpd(&mut self, dst: XmmReg, src: XmmReg) {
        self.emit(0x66);
        self.emit(0x0F);
        self.emit(0x57);
        self.emit(Self::modrm_rr(dst as u8, src as u8));
    }

    // ─── NaN-box Helpers ────────────────────────────────────────────────
    //
    // These emit inline sequences to unbox/rebox NaN-boxed Values.
    // Slot addressing: [R14 + slot * 8]

    /// Load the raw 64-bit Value bits from slot into `dst_gpr`.
    pub fn load_slot(&mut self, dst: Reg64, slot: u8) {
        self.mov_rm(dst, SLOTS_REG, slot as i32 * 8);
    }

    /// Store the raw 64-bit bits from `src_gpr` into slot.
    pub fn store_slot(&mut self, slot: u8, src: Reg64) {
        self.mov_mr(SLOTS_REG, slot as i32 * 8, src);
    }

    /// Unbox an integer Value from `slot` into `dst_gpr`.
    /// Extracts the 48-bit payload and sign-extends to 64-bit.
    /// Layout: QNAN | TAG_INT | 48-bit payload
    /// We need: shl 16; sar 16 to sign-extend the lower 48 bits.
    pub fn unbox_int(&mut self, slot: u8, dst: Reg64) {
        self.load_slot(dst, slot);
        // shl dst, 16
        self.shl_ri8(dst, 16);
        // sar dst, 16 (arithmetic right shift to sign-extend)
        self.emit(Self::rex_b(dst));
        self.emit(0xC1);
        self.emit(Self::modrm_rr(7, dst as u8));
        self.emit(16);
    }

    /// Rebox an integer from `src_gpr` into `slot`.
    /// Creates QNAN | TAG_INT | (value & 0x0000FFFFFFFFFFFF)
    pub fn rebox_int(&mut self, src: Reg64, slot: u8) {
        // mov r15, QNAN | TAG_INT
        self.mov_ri64(Reg64::R15, QNAN | TAG_INT);
        // Move src to RBX to preserve it
        self.mov_rr(Reg64::RBX, src);
        // Mask the lower 48 bits: and rbx, 0x0000FFFFFFFFFFFF
        self.mov_ri64(Reg64::R11, 0x0000FFFFFFFFFFFF);
        self.and_rr(Reg64::RBX, Reg64::R11);
        // or r15, rbx
        self.or_rr(Reg64::R15, Reg64::RBX);
        // Store to slot
        self.store_slot(slot, Reg64::R15);
    }

    /// Unbox a float Value from `slot` into XMM register.
    /// Float values are stored as raw f64 bits (no tag needed, just load directly).
    pub fn unbox_float(&mut self, slot: u8, dst: XmmReg) {
        self.movsd_load(dst, SLOTS_REG, slot as i32 * 8);
    }

    /// Rebox a float from XMM register into `slot`.
    /// Just store the raw f64 bits.
    pub fn rebox_float(&mut self, src: XmmReg, slot: u8) {
        self.movsd_store(SLOTS_REG, slot as i32 * 8, src);
    }

    /// Box a boolean (0 or 1 in `src_gpr`) into `slot`.
    /// 0 → TAG_FALSE, 1 → TAG_TRUE
    pub fn rebox_bool(&mut self, src: Reg64, slot: u8) {
        // We have 0 or 1 in src. We want:
        // result = QNAN | (src == 0 ? TAG_FALSE : TAG_TRUE)
        // TAG_FALSE = 0x0002000000000000, TAG_TRUE = 0x0003000000000000
        // TAG_FALSE + 1*0x0001000000000000 = TAG_TRUE
        // So: result = QNAN | TAG_FALSE | (src << 48)
        //
        // Simpler: load both constants and select with test+cmov
        self.mov_ri64(Reg64::R15, QNAN | TAG_FALSE);
        self.mov_ri64(Reg64::R11, QNAN | TAG_TRUE);
        // test src, src
        self.test_rr(src, src);
        // cmovne r15, r11 (if src != 0, use TRUE)
        self.emit(Self::rex_rr(Reg64::R15, Reg64::R11));
        self.emit(0x0F);
        self.emit(0x45); // cmovne
        self.emit(Self::modrm_rr(Reg64::R15 as u8, Reg64::R11 as u8));
        // Store to slot
        self.store_slot(slot, Reg64::R15);
    }

    /// Emit a deopt return: load Value::deopt(bc_ip) into RAX and jump to epilogue.
    /// `epilogue_offset` is the code offset of the epilogue, will be patched via relocation.
    pub fn emit_deopt(&mut self, bc_ip: usize) -> usize {
        // mov rax, QNAN | TAG_DEOPT | (bc_ip & 0x0000FFFFFFFFFFFF)
        let deopt_bits = QNAN | TAG_DEOPT | (bc_ip as u64 & 0x0000FFFFFFFFFFFF);
        self.mov_ri64(Reg64::RAX, deopt_bits);
        // jmp to epilogue (returns patch offset for relocation)
        self.jmp_rel32()
    }

    /// Emit a return: load value from slot into RAX and jump to epilogue.
    pub fn emit_return(&mut self, src_slot: u8) -> usize {
        self.load_slot(Reg64::RAX, src_slot);
        self.jmp_rel32()
    }

    /// Emit inline type guard for integer: check that slot contains TAG_INT.
    /// If the check fails, jumps to deopt. Returns patch offset for the deopt jump.
    pub fn emit_int_guard(&mut self, slot: u8) -> usize {
        // Load value bits
        self.load_slot(Reg64::R11, slot);
        // Extract tag: shr r11, 48 -> get upper 16 bits
        self.shr_ri8(Reg64::R11, 48);
        // Compare with TAG_INT >> 48 = 0x7FFC
        let tag_int_hi = ((QNAN | TAG_INT) >> 48) as i32;
        // cmp r11d, tag_int_hi
        self.emit(0x41); // REX.B for R11
        self.emit(0x81);
        self.emit(Self::modrm_rr(7, Reg64::R11 as u8)); // /7 = cmp
        self.emit_i32(tag_int_hi);
        // jne -> deopt
        self.jcc_rel32(Cc::NE)
    }

    /// Emit inline check if slot is falsey for JumpIfFalse.
    /// Checks: null, false, int 0. Sets ZF if falsey.
    /// After this, use JE (jump if equal/zero) for "if falsey" branch.
    pub fn emit_falsey_test(&mut self, slot: u8) {
        self.load_slot(Reg64::RAX, slot);
        // Compare against known falsey values:
        // null = QNAN | TAG_NULL
        self.mov_ri64(Reg64::R11, QNAN | TAG_NULL);
        self.cmp_rr(Reg64::RAX, Reg64::R11);
        let skip_null = self.jcc_rel32(Cc::E);

        // false = QNAN | TAG_FALSE
        self.mov_ri64(Reg64::R11, QNAN | TAG_FALSE);
        self.cmp_rr(Reg64::RAX, Reg64::R11);
        let skip_false = self.jcc_rel32(Cc::E);

        // int 0 = QNAN | TAG_INT | 0
        self.mov_ri64(Reg64::R11, QNAN | TAG_INT);
        self.cmp_rr(Reg64::RAX, Reg64::R11);
        let skip_int0 = self.jcc_rel32(Cc::E);

        // Not falsey — clear ZF by doing test with nonzero
        self.mov_ri32(Reg64::R11, 1);
        self.test_rr(Reg64::R11, Reg64::R11); // Sets ZF=0 (not zero)
        let skip_end = self.jmp_rel32();

        // Falsey target — set ZF by xor r11,r11; test r11,r11
        let falsey_target = self.pos();
        self.mov_ri32(Reg64::R11, 0);
        self.test_rr(Reg64::R11, Reg64::R11); // Sets ZF=1 (zero)

        let end_target = self.pos();

        // Patch all the jumps to falsey_target
        self.patch_jmp(skip_null, falsey_target);
        self.patch_jmp(skip_false, falsey_target);
        self.patch_jmp(skip_int0, falsey_target);
        self.patch_jmp(skip_end, end_target);
    }

    /// Patch a previously emitted rel32 jump to point to `target`.
    pub fn patch_jmp(&mut self, patch_offset: usize, target: usize) {
        let rel = (target as i64 - (patch_offset as i64 + 4)) as i32;
        let bytes = rel.to_le_bytes();
        self.code[patch_offset..patch_offset + 4].copy_from_slice(&bytes);
    }
}
