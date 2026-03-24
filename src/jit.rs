use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use crate::value::{Function, Value};
use crate::chunk::OpCode;
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// DESIGN NOTES
//
// In Quin's compiler, `Stmt::Let` does NOT emit SetLocal.
// The initializer value is pushed onto the stack and stays at a fixed position.
// Local `i` = stack slot 1 = vars[1]. Local `sum` = stack slot 2 = vars[2].
//
// Therefore: start_depth = arity + 1 (slot 0 = closure, slots 1..arity = args).
// The initial Constant(0) ops for `let i=0` and `let sum=0` write to vars[1]
// and vars[2] respectively because current_depth starts at 1.
//
// With max_locals=3 as start_depth (WRONG), Constant(0) writes to vars[3] and
// vars[4] — leaving vars[1] and vars[2] as null. GetLocal(1) reads null.
// This causes Cranelift to fail silently (define_function error → null_ptr),
// so the interpreter runs instead, producing the ~1.7s result.
//
// QUASAR JIT — current opcode support:
//   integers: Constant, GetLocal, SetLocal, Add/Sub/Mul/Div, Equal/Greater/Less
//             JumpIfFalse, Jump, Loop, Return, Negate, Not
//   floats:   same ops with ProvenFloat — fadd/fsub/fmul/fdiv + fcmp
//   other:    GetGlobal (placeholder), Call (bail — no stack materialization yet)
//   pending:  GetProperty/SetProperty, closures, stack materialization for Call
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum JitType { Unknown, ProvenInt, ProvenFloat }

pub struct JitEngine {
    ctx: codegen::Context,
    module: JITModule,
    fn_counter: usize,
}

impl JitEngine {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        flag_builder.set("opt_level", "speed").unwrap();
        let isa_builder = cranelift_native::builder()
            .unwrap_or_else(|msg| panic!("host machine unsupported: {}", msg));
        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();
        let module = JITModule::new(
            JITBuilder::with_isa(isa, cranelift_module::default_libcall_names())
        );
        Self { ctx: codegen::Context::new(), module, fn_counter: 0 }
    }

    pub fn compile(&mut self, function: &Function) -> *const u8 {
        self.module.clear_context(&mut self.ctx);

        let ptr_type = self.module.target_config().pointer_type();
        let mut sig  = self.module.make_signature();
        sig.params.push(AbiParam::new(ptr_type)); // *mut VM
        sig.params.push(AbiParam::new(ptr_type)); // *const Value (args)
        sig.returns.push(AbiParam::new(types::I64));
        self.ctx.func.signature = sig;

        // NaN boxing layout
        let qnan:         u64 = 0x7ff8000000000000;
        let tag_int:      u64 = 0x0004000000000000;
        let tag_deopt:    u64 = 0x0007000000000000;
        let tag_null:     u64 = 0x0001000000000000;
        let tag_false:    u64 = 0x0002000000000000;
        let tag_true:     u64 = 0x0003000000000000;
        let int_mask:     u64 = 0xFFFF000000000000;
        let int_prefix:   u64 = qnan | tag_int;
        let payload_mask: u64 = 0x0000FFFFFFFFFFFF;
        let null_raw:     i64 = (qnan | tag_null)  as i64;
        let false_raw:    i64 = (qnan | tag_false) as i64;
        let true_raw:     i64 = (qnan | tag_true)  as i64;

        let chunk = &function.chunk;

        // CRITICAL: start_depth = arity + 1, NOT max_locals.
        // Stmt::Let pushes a value onto the eval stack WITHOUT emitting SetLocal.
        // The value lives at the next available stack slot above the args.
        // GetLocal(1) always refers to stack slot 1 = vars[1].
        let start_depth = function.arity + 1;
        let var_types   = self.infer_types(function, start_depth);

        // Loop header detection
        let mut loop_headers = std::collections::HashSet::new();
        for (ip, op) in chunk.code.iter().enumerate() {
            if let OpCode::Loop(off) = op {
                loop_headers.insert((ip + 1).wrapping_sub(*off));
            }
        }

        // ── PASS 1: Block discovery ───────────────────────────────────────────
        let mut block_starts = std::collections::BTreeSet::new();
        block_starts.insert(0usize);
        for ip in 0..chunk.code.len() {
            match &chunk.code[ip] {
                OpCode::JumpIfFalse(off) => {
                    block_starts.insert(ip + 1);
                    block_starts.insert(ip + 1 + off);
                }
                OpCode::Jump(off) => {
                    block_starts.insert(ip + 1 + off);
                    if ip + 1 < chunk.code.len() { block_starts.insert(ip + 1); }
                }
                OpCode::Loop(off) => {
                    block_starts.insert((ip + 1).wrapping_sub(*off));
                    if ip + 1 < chunk.code.len() { block_starts.insert(ip + 1); }
                }
                OpCode::Return => {
                    if ip + 1 < chunk.code.len() { block_starts.insert(ip + 1); }
                }
                _ => {}
            }
        }

        let mut bcx = FunctionBuilderContext::new();
        let mut b   = FunctionBuilder::new(&mut self.ctx.func, &mut bcx);

        // FIX: separate real_entry — NEVER in ip_to_block
        let real_entry = b.create_block();
        b.append_block_params_for_function_params(real_entry);

        let mut ip_to_block:      HashMap<usize, Block> = HashMap::new();
        let mut ip_to_pre_header: HashMap<usize, Block> = HashMap::new();
        for &ip in &block_starts {
            ip_to_block.insert(ip, b.create_block());
            if loop_headers.contains(&ip) {
                ip_to_pre_header.insert(ip, b.create_block());
            }
        }

        // ── PASS 2: Stack depths with correct peek semantics ──────────────────
        // SetLocal: PEEK (value stays on stack, depth unchanged)
        // JumpIfFalse: PEEK (condition bool stays in both branches)
        let mut ip_to_depth: HashMap<usize, usize> = HashMap::new();
        ip_to_depth.insert(0, start_depth);
        {
            let mut wl = vec![0usize];
            while let Some(ip) = wl.pop() {
                let d = ip_to_depth[&ip];
                let push = |m: &mut HashMap<usize,usize>, w: &mut Vec<usize>, nip, nd: usize| {
                    if nip < chunk.code.len() && !m.contains_key(&nip) {
                        m.insert(nip, nd); w.push(nip);
                    }
                };
                match &chunk.code[ip] {
                    OpCode::Return => {}
                    OpCode::Jump(off)  => push(&mut ip_to_depth, &mut wl, ip+1+off, d),
                    OpCode::Loop(off)  => push(&mut ip_to_depth, &mut wl, (ip+1).wrapping_sub(*off), d),
                    OpCode::JumpIfFalse(off) => {
                        push(&mut ip_to_depth, &mut wl, ip+1,      d); // peek: bool stays
                        push(&mut ip_to_depth, &mut wl, ip+1+off,  d); // peek: bool stays
                    }
                    op => {
                        let nd = match op {
                            OpCode::Constant(_)|OpCode::Null|OpCode::True|OpCode::False
                            |OpCode::Dup|OpCode::GetLocal(_) => d + 1,
                            OpCode::SetLocal(_) => d,         // PEEK
                            OpCode::Pop         => d.saturating_sub(1),
                            OpCode::Add|OpCode::Subtract|OpCode::Multiply|OpCode::Divide
                            |OpCode::Equal|OpCode::Greater|OpCode::Less => d.saturating_sub(1),
                            _ => d,
                        };
                        push(&mut ip_to_depth, &mut wl, ip+1, nd);
                    }
                }
            }
        }

        // Block params: one i64 per live stack slot
        for (&ip, &blk) in &ip_to_block {
            let d = *ip_to_depth.get(&ip).unwrap_or(&start_depth);
            for _ in 0..d { b.append_block_param(blk, types::I64); }
            if let Some(&pre) = ip_to_pre_header.get(&ip) {
                for _ in 0..d { b.append_block_param(pre, types::I64); }
            }
        }

        // ── Entry block ───────────────────────────────────────────────────────
        b.switch_to_block(real_entry);
        let args_ptr = b.block_params(real_entry)[1];

        // SSA vars: enough for all locals + eval-stack headroom
        let num_vars = (start_depth + function.max_locals + 64).max(128);
        let vars: Vec<Variable> = (0..num_vars).map(|i| {
            let v = Variable::new(i); b.declare_var(v, types::I64); v
        }).collect();

        let mut var_is_raw  = vec![false;            num_vars];
        let mut slot_types  = vec![JitType::Unknown; num_vars];

        // Precompute constants in real_entry (valid everywhere due to dominance)
        let c_null   = b.ins().iconst(types::I64, null_raw);
        let c_false  = b.ins().iconst(types::I64, false_raw);
        let c_true   = b.ins().iconst(types::I64, true_raw);
        let c_pmask  = b.ins().iconst(types::I64, payload_mask as i64);
        let c_imask  = b.ins().iconst(types::I64, int_mask     as i64);
        let c_prefix = b.ins().iconst(types::I64, int_prefix   as i64);

        // Load args (closure + function parameters) from args_ptr
        for i in 0..=function.arity {
            let tagged = b.ins().load(types::I64, MemFlags::new(), args_ptr, (i * 8) as i32);
            if i < var_types.len() && var_types[i] == JitType::ProvenInt {
                let raw = b.ins().band(tagged, c_pmask);
                b.def_var(vars[i], raw);
                var_is_raw[i] = true;
                slot_types[i] = JitType::ProvenInt;
            } else {
                b.def_var(vars[i], tagged);
                slot_types[i] = JitType::Unknown;
            }
        }
        // All other slots start as null; will be initialized by bytecode
        for i in function.arity + 1..num_vars {
            b.def_var(vars[i], c_null);
        }

        // Jump from real_entry to ip=0 block
        {
            let d0   = *ip_to_depth.get(&0).unwrap_or(&start_depth);
            let args: Vec<cranelift::prelude::Value> = (0..d0).map(|i| b.use_var(vars[i])).collect();
            let dest = ip_to_pre_header.get(&0).copied().unwrap_or(ip_to_block[&0]);
            b.ins().jump(dest, &args);
        }
        b.seal_block(real_entry);

        // ── Helpers ───────────────────────────────────────────────────────────
        macro_rules! bail {
            () => {{ return std::ptr::null(); }};
        }

        macro_rules! deopt_ret {
            ($ip:expr) => {{
                let dv = (qnan | tag_deopt | ($ip as u64 & payload_mask)) as i64;
                let dv_val = b.ins().iconst(types::I64, dv); b.ins().return_(&[dv_val]);
            }};
        }

        // Ensure slot is a raw int payload; emit guard + strip tag if needed
        macro_rules! ensure_raw_int {
            ($slot:expr, $ip:expr) => {{
                if !var_is_raw[$slot] {
                    let v   = b.use_var(vars[$slot]);
                    let tag = b.ins().band(v, c_imask);
                    let ok  = b.ins().icmp(IntCC::Equal, tag, c_prefix);
                    let ok_b = b.create_block();
                    let no_b = b.create_block();
                    b.ins().brif(ok, ok_b, &[], no_b, &[]);
                    b.switch_to_block(no_b); deopt_ret!($ip);
                    b.seal_block(no_b);
                    b.switch_to_block(ok_b);
                    b.seal_block(ok_b);
                    let v2  = b.use_var(vars[$slot]);
                    let raw = b.ins().band(v2, c_pmask);
                    b.def_var(vars[$slot], raw);
                    var_is_raw[$slot] = true;
                    slot_types[$slot] = JitType::ProvenInt;
                }
            }};
        }

        macro_rules! get_raw {
            ($s:expr) => {{
                let v = b.use_var(vars[$s]);
                if var_is_raw[$s] { v } else { b.ins().band(v, c_pmask) }
            }};
        }

        macro_rules! get_tagged {
            ($s:expr) => {{
                let v = b.use_var(vars[$s]);
                if var_is_raw[$s] { b.ins().bor(v, c_prefix) } else { v }
            }};
        }

        // Build block jump args for depth d.
        // ProvenInt slots pass raw payload; Unknown slots pass tagged.
        macro_rules! args_for {
            ($d:expr) => {{
                let mut a = Vec::with_capacity($d);
                for i in 0..$d {
                    let av_vt    = var_types.get(i).copied().unwrap_or(JitType::Unknown);
                    let want_raw = av_vt == JitType::ProvenInt || av_vt == JitType::ProvenFloat;
                    let v = b.use_var(vars[i]);
                    a.push(if want_raw && !var_is_raw[i] {
                        b.ins().band(v, c_pmask)
                    } else if !want_raw && var_is_raw[i] {
                        b.ins().bor(v, c_prefix)
                    } else { v });
                }
                a
            }};
        }

        // ── PASS 3: IR emission ───────────────────────────────────────────────
        // real_entry already emitted its terminator jump to ip_to_block[0].
        // Start with block_terminated=true so ip=0 boundary skips emitting another jump.
        let mut block_terminated = true;
        let mut current_depth    = start_depth;
        let mut last_cmp: Option<cranelift::prelude::Value> = None;

        for ip in 0..chunk.code.len() {
            // Block boundary
            if let Some(&target_blk) = ip_to_block.get(&ip) {
                let d = *ip_to_depth.get(&ip).unwrap_or(&start_depth);

                if !block_terminated {
                    // Emit forward jump from current block to target
                    let jump_d = d.min(current_depth);
                    let mut jargs = args_for!(jump_d);
                    // Pad missing slots with c_null
                    while jargs.len() < d { jargs.push(c_null); }
                    let dest = ip_to_pre_header.get(&ip).copied().unwrap_or(target_blk);
                    b.ins().jump(dest, &jargs);
                }

                // Pre-header for loop headers
                if let Some(&pre) = ip_to_pre_header.get(&ip) {
                    b.switch_to_block(pre);
                    let params = b.block_params(pre).to_vec();
                    for i in 0..d {
                        b.def_var(vars[i], params[i]);
                        let bv_vt = var_types.get(i).copied().unwrap_or(JitType::Unknown);
                        var_is_raw[i] = bv_vt == JitType::ProvenInt || bv_vt == JitType::ProvenFloat;
                        slot_types[i] = bv_vt;
                    }
                    // Hoist guards for any ProvenInt function args
                    for i in 0..=function.arity {
                        if i < var_types.len() && var_types[i] == JitType::ProvenInt {
                            let off    = (i * 8) as i32;
                            let tagged = b.ins().load(types::I64, MemFlags::new(), args_ptr, off);
                            let tag    = b.ins().band(tagged, c_imask);
                            let ok     = b.ins().icmp(IntCC::Equal, tag, c_prefix);
                            let ok_b   = b.create_block();
                            let no_b   = b.create_block();
                            b.ins().brif(ok, ok_b, &[], no_b, &[]);
                            b.switch_to_block(no_b); deopt_ret!(ip);
                            b.seal_block(no_b);
                            b.switch_to_block(ok_b);
                            b.seal_block(ok_b);
                        }
                    }
                    let hargs = args_for!(d);
                    b.ins().jump(target_blk, &hargs);
                    b.seal_block(pre);
                }

                b.switch_to_block(target_blk);
                block_terminated = false;
                current_depth    = d;
                let params = b.block_params(target_blk).to_vec();
                for i in 0..d {
                    b.def_var(vars[i], params[i]);
                    let bv_vt2 = var_types.get(i).copied().unwrap_or(JitType::Unknown);
                    var_is_raw[i] = bv_vt2 == JitType::ProvenInt || bv_vt2 == JitType::ProvenFloat;
                    slot_types[i] = bv_vt2;
                }
            }

            if block_terminated { continue; }
            let op = &chunk.code[ip];
            let mut is_cmp = false;

            match op {
                OpCode::Null => {
                    b.def_var(vars[current_depth], c_null);
                    var_is_raw[current_depth] = false;
                    slot_types[current_depth] = JitType::Unknown;
                    current_depth += 1;
                }
                OpCode::True => {
                    b.def_var(vars[current_depth], c_true);
                    var_is_raw[current_depth] = false;
                    slot_types[current_depth] = JitType::Unknown;
                    current_depth += 1;
                }
                OpCode::False => {
                    b.def_var(vars[current_depth], c_false);
                    var_is_raw[current_depth] = false;
                    slot_types[current_depth] = JitType::Unknown;
                    current_depth += 1;
                }
                OpCode::Constant(idx) => {
                    let bits = chunk.constants[*idx].0;
                    let val  = Value(bits);
                    if val.is_int() {
                        let cv = b.ins().iconst(types::I64, val.as_int()); b.def_var(vars[current_depth], cv);
                        var_is_raw[current_depth] = true;
                        slot_types[current_depth] = JitType::ProvenInt;
                    } else if val.is_float() {
                        // Float: store raw f64 bits as i64 — no NaN-boxing tag needed.
                        // Floats don't overlap with QNAN tags so raw bits are valid.
                        let cv = b.ins().iconst(types::I64, bits as i64); b.def_var(vars[current_depth], cv);
                        var_is_raw[current_depth] = true;   // raw f64 bits
                        slot_types[current_depth] = JitType::ProvenFloat;
                    } else {
                        let cv = b.ins().iconst(types::I64, bits as i64); b.def_var(vars[current_depth], cv);
                        var_is_raw[current_depth] = false;
                        slot_types[current_depth] = JitType::Unknown;
                    }
                    current_depth += 1;
                }

                OpCode::Pop => {
                    // Don't pop below the locals baseline (would corrupt local slots)
                    if current_depth > start_depth { current_depth -= 1; }
                }
                OpCode::Dup => {
                    if current_depth > 0 {
                        let v = b.use_var(vars[current_depth - 1]);
                        b.def_var(vars[current_depth], v);
                        var_is_raw[current_depth]  = var_is_raw[current_depth - 1];
                        slot_types[current_depth]  = slot_types[current_depth - 1];
                        current_depth += 1;
                    }
                }

                OpCode::GetLocal(idx) => {
                    if *idx >= num_vars { bail!(); }
                    let v = b.use_var(vars[*idx]);
                    b.def_var(vars[current_depth], v);
                    var_is_raw[current_depth]  = var_is_raw[*idx];
                    slot_types[current_depth]  = slot_types[*idx];
                    current_depth += 1;
                }
                // SetLocal: PEEK — reads top, writes to local, depth UNCHANGED
                OpCode::SetLocal(idx) => {
                    if current_depth == 0 || *idx >= num_vars { bail!(); }
                    let top   = current_depth - 1;
                    let top_v = b.use_var(vars[top]);
                    let idx_vt   = var_types.get(*idx).copied().unwrap_or(JitType::Unknown);
                    let want_raw = idx_vt == JitType::ProvenInt || idx_vt == JitType::ProvenFloat;
                    let stored = if want_raw && !var_is_raw[top] {
                        b.ins().band(top_v, c_pmask)
                    } else if !want_raw && var_is_raw[top] {
                        b.ins().bor(top_v, c_prefix)
                    } else { top_v };
                    b.def_var(vars[*idx], stored);
                    var_is_raw[*idx]  = want_raw;
                    slot_types[*idx]  = idx_vt;
                    // depth unchanged (peek)
                }

                // Arithmetic: fully unboxed for ProvenInt; bitcast path for ProvenFloat
                OpCode::Add | OpCode::Subtract | OpCode::Multiply | OpCode::Divide => {
                    if current_depth < 2 { bail!(); }
                    let a  = current_depth - 2;
                    let b_ = current_depth - 1;
                    let is_float = slot_types[a] == JitType::ProvenFloat
                                || slot_types[b_] == JitType::ProvenFloat;

                    let (result, result_type) = if is_float {
                        // Float path: bitcast i64→f64, compute, bitcast f64→i64
                        let av_i = b.use_var(vars[a]);
                        let bv_i = b.use_var(vars[b_]);
                        let av_f = if slot_types[a] == JitType::ProvenFloat {
                            b.ins().bitcast(types::F64, MemFlags::new(), av_i)
                        } else {
                            // int → float promotion
                            let raw = if var_is_raw[a] { av_i } else { b.ins().band(av_i, c_pmask) };
                            b.ins().fcvt_from_sint(types::F64, raw)
                        };
                        let bv_f = if slot_types[b_] == JitType::ProvenFloat {
                            b.ins().bitcast(types::F64, MemFlags::new(), bv_i)
                        } else {
                            let raw = if var_is_raw[b_] { bv_i } else { b.ins().band(bv_i, c_pmask) };
                            b.ins().fcvt_from_sint(types::F64, raw)
                        };
                        let fr = match op {
                            OpCode::Add      => b.ins().fadd(av_f, bv_f),
                            OpCode::Subtract => b.ins().fsub(av_f, bv_f),
                            OpCode::Multiply => b.ins().fmul(av_f, bv_f),
                            OpCode::Divide   => b.ins().fdiv(av_f, bv_f),
                            _ => unreachable!(),
                        };
                        (b.ins().bitcast(types::I64, MemFlags::new(), fr), JitType::ProvenFloat)
                    } else if slot_types[a] == JitType::ProvenInt && slot_types[b_] == JitType::ProvenInt {
                        // Fully unboxed integer path — zero tag overhead
                        let av = get_raw!(a);
                        let bv = get_raw!(b_);
                        let r = match op {
                            OpCode::Add      => b.ins().iadd(av, bv),
                            OpCode::Subtract => b.ins().isub(av, bv),
                            OpCode::Multiply => b.ins().imul(av, bv),
                            OpCode::Divide   => b.ins().sdiv(av, bv),
                            _ => unreachable!(),
                        };
                        (r, JitType::ProvenInt)
                    } else {
                        ensure_raw_int!(a,  ip);
                        ensure_raw_int!(b_, ip);
                        let av = get_raw!(a);
                        let bv = get_raw!(b_);
                        let r = match op {
                            OpCode::Add      => b.ins().iadd(av, bv),
                            OpCode::Subtract => b.ins().isub(av, bv),
                            OpCode::Multiply => b.ins().imul(av, bv),
                            OpCode::Divide   => b.ins().sdiv(av, bv),
                            _ => unreachable!(),
                        };
                        (r, JitType::ProvenInt)
                    };
                    current_depth -= 1;
                    b.def_var(vars[current_depth - 1], result);
                    var_is_raw[current_depth - 1]  = true;
                    slot_types[current_depth - 1]  = result_type;
                }

                OpCode::Equal | OpCode::Greater | OpCode::Less => {
                    if current_depth < 2 { bail!(); }
                    let a  = current_depth - 2;
                    let bx = current_depth - 1;
                    let is_float_cmp = slot_types[a] == JitType::ProvenFloat
                                    || slot_types[bx] == JitType::ProvenFloat;
                    let cond = if is_float_cmp {
                        let av_i = b.use_var(vars[a]);
                        let bv_i = b.use_var(vars[bx]);
                        let av_f = b.ins().bitcast(types::F64, MemFlags::new(), av_i);
                        let bv_f = b.ins().bitcast(types::F64, MemFlags::new(), bv_i);
                        let fcc = if matches!(op, OpCode::Equal)   { FloatCC::Equal }
                                  else if matches!(op, OpCode::Greater) { FloatCC::GreaterThan }
                                  else { FloatCC::LessThan };
                        b.ins().fcmp(fcc, av_f, bv_f)
                    } else if matches!(op, OpCode::Equal) {
                        if slot_types[a] == JitType::ProvenInt && slot_types[bx] == JitType::ProvenInt {
                            let av = get_raw!(a); let bv = get_raw!(bx); b.ins().icmp(IntCC::Equal, av, bv)
                        } else {
                            let av = get_tagged!(a); let bv = get_tagged!(bx); b.ins().icmp(IntCC::Equal, av, bv)
                        }
                    } else {
                        ensure_raw_int!(a,  ip);
                        ensure_raw_int!(bx, ip);
                        let cc = if matches!(op, OpCode::Greater) { IntCC::SignedGreaterThan }
                                 else { IntCC::SignedLessThan };
                        let av = get_raw!(a); let bv = get_raw!(bx); b.ins().icmp(cc, av, bv)
                    };
                    last_cmp = Some(cond);
                    is_cmp   = true;
                    // FIX: store c_null as placeholder — JumpIfFalse uses cond directly
                    // (no select overhead, no bool object materialization)
                    current_depth -= 1;
                    b.def_var(vars[current_depth - 1], c_null);
                    var_is_raw[current_depth - 1]  = false;
                    slot_types[current_depth - 1]  = JitType::Unknown;
                }

                OpCode::Not => {
                    if current_depth == 0 { bail!(); }
                    let s       = current_depth - 1;
                    let v       = get_tagged!(s);
                    let is_null = b.ins().icmp(IntCC::Equal, v, c_null);
                    let is_f    = b.ins().icmp(IntCC::Equal, v, c_false);
                    let falsey  = b.ins().bor(is_null, is_f);
                    let not_rv = b.ins().select(falsey, c_true, c_false); b.def_var(vars[s], not_rv);
                    var_is_raw[s]  = false;
                    slot_types[s]  = JitType::Unknown;
                }
                OpCode::Negate => {
                    if current_depth == 0 { bail!(); }
                    let s = current_depth - 1;
                    if slot_types[s] == JitType::ProvenFloat {
                        let vi = b.use_var(vars[s]);
                        let vf = b.ins().bitcast(types::F64, MemFlags::new(), vi);
                        let nf = b.ins().fneg(vf);
                        let ni = b.ins().bitcast(types::I64, MemFlags::new(), nf);
                        b.def_var(vars[s], ni);
                        var_is_raw[s]  = true;
                        slot_types[s]  = JitType::ProvenFloat;
                    } else {
                        if slot_types[s] != JitType::ProvenInt { ensure_raw_int!(s, ip); }
                        let raw    = get_raw!(s);
                        let neg    = b.ins().ineg(raw);
                        let masked = b.ins().band(neg, c_pmask);
                        b.def_var(vars[s], masked);
                        var_is_raw[s]  = true;
                        slot_types[s]  = JitType::ProvenInt;
                    }
                }

                // JumpIfFalse: PEEK — both branches carry the same depth
                OpCode::JumpIfFalse(offset) => {
                    if current_depth == 0 { bail!(); }
                    let target_ip = ip + 1 + offset;
                    let fall_ip   = ip + 1;
                    let td = *ip_to_depth.get(&target_ip).unwrap_or(&start_depth);
                    let fd = *ip_to_depth.get(&fall_ip).unwrap_or(&start_depth);

                    let t_args = args_for!(td);
                    let f_args = args_for!(fd);
                    let t_dest = ip_to_pre_header.get(&target_ip).copied()
                                 .unwrap_or(ip_to_block[&target_ip]);
                    let f_dest = ip_to_pre_header.get(&fall_ip).copied()
                                 .unwrap_or(ip_to_block[&fall_ip]);

                    if let Some(cond) = last_cmp {
                        // FIX: direct brif on icmp result — zero overhead
                        // cond=true → truthy → fall-through; cond=false → exit
                        b.ins().brif(cond, f_dest, &f_args, t_dest, &t_args);
                    } else {
                        let v       = get_tagged!(current_depth - 1);
                        let is_null = b.ins().icmp(IntCC::Equal, v, c_null);
                        let is_f    = b.ins().icmp(IntCC::Equal, v, c_false);
                        let falsey  = b.ins().bor(is_null, is_f);
                        b.ins().brif(falsey, t_dest, &t_args, f_dest, &f_args);
                    }
                    block_terminated = true;
                }

                OpCode::Jump(offset) => {
                    let target_ip = ip + 1 + offset;
                    let d    = *ip_to_depth.get(&target_ip).unwrap_or(&start_depth);
                    let args = args_for!(d);
                    let dest = ip_to_pre_header.get(&target_ip).copied()
                               .unwrap_or(ip_to_block[&target_ip]);
                    b.ins().jump(dest, &args);
                    block_terminated = true;
                }

                OpCode::Loop(offset) => {
                    let target_ip = (ip + 1).wrapping_sub(*offset);
                    let d    = *ip_to_depth.get(&target_ip).unwrap_or(&start_depth);
                    let args = args_for!(d);
                    // Loop always skips the pre-header (direct to header)
                    b.ins().jump(ip_to_block[&target_ip], &args);
                    block_terminated = true;
                }

                OpCode::Return => {
                    let rv = if current_depth > 0 {
                        let s = current_depth - 1;
                        if slot_types[s] == JitType::ProvenFloat {
                            // Float: raw f64 bits returned as-is — interpreter sees them
                            // via Value::is_float() which checks (bits & QNAN) != QNAN
                            b.use_var(vars[s])
                        } else if var_is_raw[s] {
                            // Int: re-box raw payload by OR-ing the int tag prefix
                            let v = b.use_var(vars[s]);
                            b.ins().bor(v, c_prefix)
                        } else {
                            b.use_var(vars[s])
                        }
                    } else {
                        c_null
                    };
                    b.ins().return_(&[rv]);
                    block_terminated = true;
                }
                OpCode::GetGlobal(_) => {
                    // Push Unknown placeholder — globals resolved by interpreter
                    b.def_var(vars[current_depth], c_null);
                    var_is_raw[current_depth] = false;
                    slot_types[current_depth] = JitType::Unknown;
                    current_depth += 1;
                }

                OpCode::Call(_) => {
                    // bail — SSA vars not materialized to stack yet
                    // any function with a Call runs fully interpreted
                    // TODO: stack materialization → recursive JIT
                    bail!();
                }

                _ => bail!(), // unsupported opcode — fall back to interpreter
            }

            if !is_cmp { last_cmp = None; }
        }

        if !block_terminated { b.ins().return_(&[c_null]); }

        // Seal all bytecode blocks (done AFTER all opcodes so back-edges are emitted)
        for (_, &blk) in &ip_to_block { b.seal_block(blk); }
        b.finalize();

        let name = format!("quin_{}_{}", function.name, self.fn_counter);
        self.fn_counter += 1;

        let id = match self.module.declare_function(&name, Linkage::Export, &self.ctx.func.signature) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("[JIT] declare_function failed for {}: {:?}", function.name, e);
                return std::ptr::null();
            }
        };
        if let Err(e) = self.module.define_function(id, &mut self.ctx) {
            eprintln!("[JIT] define_function failed for {}: {:?}", function.name, e);
            return std::ptr::null();
        }
        if let Err(e) = self.module.finalize_definitions() {
            eprintln!("[JIT] finalize_definitions failed for {}: {:?}", function.name, e);
            return std::ptr::null();
        }

        self.module.get_finalized_function(id)
    }

    fn infer_types(&self, function: &Function, start_depth: usize) -> Vec<JitType> {
        let n = (start_depth + function.max_locals + 64).max(128);
        let mut vt = vec![JitType::ProvenInt; n];
        let chunk = &function.chunk;
        // Function args/closure are Unknown type (caller decides)
        for i in 0..=function.arity { vt[i] = JitType::Unknown; }

        // Fixed-point: propagate type constraints
        let mut changed = true;
        while changed {
            changed = false;
            let mut stype: Vec<JitType> = (0..n).map(|i| vt[i]).collect();
            let mut d = start_depth;

            for op in &chunk.code {
                let set = |stype: &mut Vec<JitType>, idx: usize, t: JitType| {
                    if idx < stype.len() { stype[idx] = t; }
                };
                match op {
                    OpCode::Constant(idx) => {
                        let raw = chunk.constants[*idx].0;
                        let t = if Value(raw).is_int() { JitType::ProvenInt }
                                else if Value(raw).is_float() { JitType::ProvenFloat }
                                else { JitType::Unknown };
                        set(&mut stype, d, t); d += 1;
                    }
                    OpCode::Null|OpCode::True|OpCode::False => { set(&mut stype, d, JitType::Unknown); d += 1; }
                    OpCode::Pop => { if d > start_depth { d -= 1; } }
                    OpCode::Dup => {
                        let t = if d > 0 { stype.get(d-1).copied().unwrap_or(JitType::Unknown) } else { JitType::Unknown };
                        set(&mut stype, d, t); d += 1;
                    }
                    OpCode::GetLocal(i) => {
                        let t = if *i < vt.len() { vt[*i] } else { JitType::Unknown };
                        set(&mut stype, d, t); d += 1;
                    }
                    OpCode::SetLocal(i) => {
                        let t = if d > 0 { stype.get(d-1).copied().unwrap_or(JitType::Unknown) } else { JitType::Unknown };
                        if *i < vt.len() {
                            if vt[*i] == JitType::ProvenInt && t != JitType::ProvenInt {
                                vt[*i] = if t == JitType::ProvenFloat { JitType::ProvenFloat } else { JitType::Unknown };
                                changed = true;
                            } else if vt[*i] == JitType::ProvenFloat && t != JitType::ProvenFloat {
                                vt[*i] = JitType::Unknown; changed = true;
                            }
                        }
                        // PEEK — d unchanged
                    }
                    OpCode::Add|OpCode::Subtract|OpCode::Multiply|OpCode::Divide => {
                        if d >= 2 {
                            let bv = stype.get(d-1).copied().unwrap_or(JitType::Unknown);
                            let av = stype.get(d-2).copied().unwrap_or(JitType::Unknown);
                            d -= 1;
                            let r = if av == JitType::ProvenFloat || bv == JitType::ProvenFloat {
                                JitType::ProvenFloat
                            } else if av == JitType::ProvenInt && bv == JitType::ProvenInt {
                                JitType::ProvenInt
                            } else { JitType::Unknown };
                            set(&mut stype, d-1, r);
                        }
                    }
                    OpCode::Equal|OpCode::Greater|OpCode::Less => {
                        if d >= 2 { d -= 1; set(&mut stype, d-1, JitType::Unknown); }
                    }
                    OpCode::Not|OpCode::Negate => { if d > 0 { set(&mut stype, d-1, JitType::Unknown); } }
                    OpCode::JumpIfFalse(_) => {} // PEEK — no change
                    OpCode::Return => { d = start_depth; }
                    _ => {}
                }
            }
        }
        vt
    }
}