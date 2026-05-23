use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};
use crate::value::{Function, Value};
use crate::frontend::chunk::OpCode;
use std::collections::HashMap;
use super::JitEngine;
use super::types::JitType;

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
// Ame, current opcode support:
//   integers: Constant, GetLocal, SetLocal, Add/Sub/Mul/Div, Equal/Greater/Less
//             JumpIfFalse, Jump, Loop, Return, Negate, Not
//   floats:   same ops with ProvenFloat — fadd/fsub/fmul/fdiv + fcmp
//   libcalls: GetIndex (array), SetIndex (array), Call(1) (native sqrt/len),
//             GetGlobal (any global variable)
//   pending:  GetProperty/SetProperty, closures, Call(>1)
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn compile_function(engine: &mut JitEngine, function: &Function) -> *const u8 {
    engine.module.clear_context(&mut engine.ctx);

    let ptr_type = engine.module.target_config().pointer_type();
    let mut sig  = engine.module.make_signature();
    sig.params.push(AbiParam::new(ptr_type)); // *mut VM
    sig.params.push(AbiParam::new(ptr_type)); // *const Value (args)
    sig.returns.push(AbiParam::new(types::I64));
    engine.ctx.func.signature = sig;

    // ── Declare libcall signatures ────────────────────────────────────
    // quin_array_get(arr_bits: i64, idx_bits: i64) -> i64
    let mut sig_array_get = engine.module.make_signature();
    sig_array_get.params.push(AbiParam::new(types::I64));
    sig_array_get.params.push(AbiParam::new(types::I64));
    sig_array_get.returns.push(AbiParam::new(types::I64));
    let fn_array_get = engine.module.declare_function("quin_array_get", Linkage::Import, &sig_array_get)
        .expect("declare quin_array_get");

    // quin_array_set(arr_bits: i64, idx_bits: i64, val_bits: i64) -> i64
    let mut sig_array_set = engine.module.make_signature();
    sig_array_set.params.push(AbiParam::new(types::I64));
    sig_array_set.params.push(AbiParam::new(types::I64));
    sig_array_set.params.push(AbiParam::new(types::I64));
    sig_array_set.returns.push(AbiParam::new(types::I64));
    let fn_array_set = engine.module.declare_function("quin_array_set", Linkage::Import, &sig_array_set)
        .expect("declare quin_array_set");

    // quin_call_native_1(vm: ptr, fn_bits: i64, arg_bits: i64) -> i64
    let mut sig_call_native = engine.module.make_signature();
    sig_call_native.params.push(AbiParam::new(ptr_type));
    sig_call_native.params.push(AbiParam::new(types::I64));
    sig_call_native.params.push(AbiParam::new(types::I64));
    sig_call_native.returns.push(AbiParam::new(types::I64));
    let fn_call_native = engine.module.declare_function("quin_call_native_1", Linkage::Import, &sig_call_native)
        .expect("declare quin_call_native_1");

    // quin_get_global(vm: ptr, const_ptr: ptr, const_idx: i64) -> i64
    let mut sig_get_global = engine.module.make_signature();
    sig_get_global.params.push(AbiParam::new(ptr_type));
    sig_get_global.params.push(AbiParam::new(ptr_type));
    sig_get_global.params.push(AbiParam::new(types::I64));
    sig_get_global.returns.push(AbiParam::new(types::I64));
    let fn_get_global = engine.module.declare_function("quin_get_global", Linkage::Import, &sig_get_global)
        .expect("declare quin_get_global");

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
    let start_depth = function.arity + 1;
    let var_types   = engine.infer_types(function, start_depth);

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
    let mut b   = FunctionBuilder::new(&mut engine.ctx.func, &mut bcx);

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
                    push(&mut ip_to_depth, &mut wl, ip+1,      d);
                    push(&mut ip_to_depth, &mut wl, ip+1+off,  d);
                }
                op => {
                    let nd = match op {
                        OpCode::Constant(_)|OpCode::Null|OpCode::True|OpCode::False
                        |OpCode::Dup|OpCode::GetLocal(_) => d + 1,
                        OpCode::GetGlobal(_) => d + 1,
                        OpCode::SetLocal(_) => d,         // PEEK
                        OpCode::Pop         => d.saturating_sub(1),
                        OpCode::Add|OpCode::Subtract|OpCode::Multiply|OpCode::Divide
                        |OpCode::Equal|OpCode::Greater|OpCode::Less => d.saturating_sub(1),
                        OpCode::GetIndex => d.saturating_sub(1),
                        OpCode::SetIndex => d.saturating_sub(2),
                        OpCode::Call(n) => d.saturating_sub(*n as usize),
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
    let vm_ptr_val = b.block_params(real_entry)[0];
    let args_ptr = b.block_params(real_entry)[1];

    // Import libcall function references
    let fn_ref_array_get = engine.module.declare_func_in_func(fn_array_get, b.func);
    let fn_ref_array_set = engine.module.declare_func_in_func(fn_array_set, b.func);
    let fn_ref_call_native = engine.module.declare_func_in_func(fn_call_native, b.func);
    let fn_ref_get_global = engine.module.declare_func_in_func(fn_get_global, b.func);

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

    // Precompute constants pointer for get_global libcall
    let const_ptr_val = b.ins().iconst(ptr_type,
        function.chunk.constants.as_ptr() as i64);

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
    // All other slots start as null
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
            if var_is_raw[$s] {
                v
            } else {
                let masked = b.ins().band(v, c_pmask);
                let amt = b.ins().iconst(types::I32, 16);
                let shl = b.ins().ishl(masked, amt);
                b.ins().sshr(shl, amt)
            }
        }};
    }

    macro_rules! get_tagged {
        ($s:expr) => {{
            let v = b.use_var(vars[$s]);
            if var_is_raw[$s] {
                if slot_types[$s] == JitType::ProvenFloat {
                    v
                } else {
                    let masked = b.ins().band(v, c_pmask);
                    b.ins().bor(masked, c_prefix)
                }
            } else { v }
        }};
    }

    macro_rules! args_for {
        ($d:expr) => {{
            let mut a = Vec::with_capacity($d);
            for i in 0..$d {
                let av_vt    = var_types.get(i).copied().unwrap_or(JitType::Unknown);
                let want_raw = av_vt == JitType::ProvenInt || av_vt == JitType::ProvenFloat;
                let v = b.use_var(vars[i]);
                a.push(if want_raw && !var_is_raw[i] {
                    if av_vt == JitType::ProvenFloat {
                        v
                    } else {
                        let masked = b.ins().band(v, c_pmask);
                        let amt = b.ins().iconst(types::I32, 16);
                        let shl = b.ins().ishl(masked, amt);
                        b.ins().sshr(shl, amt)
                    }
                } else if !want_raw && var_is_raw[i] {
                    if slot_types[i] == JitType::ProvenFloat {
                        v
                    } else {
                        let masked = b.ins().band(v, c_pmask);
                        b.ins().bor(masked, c_prefix)
                    }
                } else { v });
            }
            a
        }};
    }

    macro_rules! get_for_libcall {
        ($s:expr) => {{
            let v = b.use_var(vars[$s]);
            if slot_types[$s] == JitType::ProvenFloat {
                v
            } else if var_is_raw[$s] {
                let masked = b.ins().band(v, c_pmask);
                b.ins().bor(masked, c_prefix)
            } else {
                v
            }
        }};
    }

    // ── PASS 3: IR emission ───────────────────────────────────────────────
    let mut block_terminated = true;
    let mut current_depth    = start_depth;
    let mut last_cmp: Option<cranelift::prelude::Value> = None;

    for ip in 0..chunk.code.len() {
        // Block boundary
        if let Some(&target_blk) = ip_to_block.get(&ip) {
            let d = *ip_to_depth.get(&ip).unwrap_or(&start_depth);

            if !block_terminated {
                let jump_d = d.min(current_depth);
                let mut jargs = args_for!(jump_d);
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
                    let cv = b.ins().iconst(types::I64, bits as i64); b.def_var(vars[current_depth], cv);
                    var_is_raw[current_depth] = true;
                    slot_types[current_depth] = JitType::ProvenFloat;
                } else {
                    let cv = b.ins().iconst(types::I64, bits as i64); b.def_var(vars[current_depth], cv);
                    var_is_raw[current_depth] = false;
                    slot_types[current_depth] = JitType::Unknown;
                }
                std::mem::forget(val);
                current_depth += 1;
            }

            OpCode::Pop => {
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
            OpCode::SetLocal(idx) => {
                if current_depth == 0 || *idx >= num_vars { bail!(); }
                let top   = current_depth - 1;
                let top_v = b.use_var(vars[top]);
                let idx_vt   = var_types.get(*idx).copied().unwrap_or(JitType::Unknown);
                let want_raw = idx_vt == JitType::ProvenInt || idx_vt == JitType::ProvenFloat;
                let stored = if want_raw && !var_is_raw[top] {
                    if idx_vt == JitType::ProvenFloat {
                        top_v
                    } else {
                        let masked = b.ins().band(top_v, c_pmask);
                        let amt = b.ins().iconst(types::I32, 16);
                        let shl = b.ins().ishl(masked, amt);
                        b.ins().sshr(shl, amt)
                    }
                } else if !want_raw && var_is_raw[top] {
                    if slot_types[top] == JitType::ProvenFloat {
                        top_v
                    } else {
                        let masked = b.ins().band(top_v, c_pmask);
                        b.ins().bor(masked, c_prefix)
                    }
                } else { top_v };
                b.def_var(vars[*idx], stored);
                var_is_raw[*idx]  = want_raw;
                slot_types[*idx]  = idx_vt;
            }

            // ── GetGlobal via libcall ─────────────────────────────────────
            OpCode::GetGlobal(idx) => {
                let idx_const = b.ins().iconst(types::I64, *idx as i64);
                let call = b.ins().call(fn_ref_get_global, &[vm_ptr_val, const_ptr_val, idx_const]);
                let result = b.inst_results(call)[0];
                b.def_var(vars[current_depth], result);
                var_is_raw[current_depth] = false;
                slot_types[current_depth] = JitType::Unknown;
                current_depth += 1;
            }

            // ── GetIndex via libcall ──────────────────────────────────────
            OpCode::GetIndex => {
                if current_depth < 2 { bail!(); }
                let idx_slot = current_depth - 1;
                let arr_slot = current_depth - 2;
                let arr_tagged = get_for_libcall!(arr_slot);
                let idx_tagged = get_for_libcall!(idx_slot);
                let call = b.ins().call(fn_ref_array_get, &[arr_tagged, idx_tagged]);
                let result = b.inst_results(call)[0];
                current_depth -= 1;
                b.def_var(vars[current_depth - 1], result);
                var_is_raw[current_depth - 1] = false;
                slot_types[current_depth - 1] = JitType::Unknown;
            }

            // ── SetIndex via libcall ──────────────────────────────────────
            OpCode::SetIndex => {
                if current_depth < 3 { bail!(); }
                let val_slot = current_depth - 1;
                let idx_slot = current_depth - 2;
                let arr_slot = current_depth - 3;
                let arr_tagged = get_for_libcall!(arr_slot);
                let idx_tagged = get_for_libcall!(idx_slot);
                let val_tagged = get_for_libcall!(val_slot);
                let call = b.ins().call(fn_ref_array_set, &[arr_tagged, idx_tagged, val_tagged]);
                let result = b.inst_results(call)[0];
                current_depth -= 2;
                b.def_var(vars[current_depth - 1], result);
                var_is_raw[current_depth - 1] = false;
                slot_types[current_depth - 1] = JitType::Unknown;
            }

            // ── Call(1) via native libcall ────────────────────────────────
            OpCode::Call(arg_count) => {
                if *arg_count == 1 {
                    if current_depth < 2 { bail!(); }
                    let fn_slot  = current_depth - 2;
                    let arg_slot = current_depth - 1;
                    let fn_tagged  = get_for_libcall!(fn_slot);
                    let arg_tagged = get_for_libcall!(arg_slot);
                    let call = b.ins().call(fn_ref_call_native, &[vm_ptr_val, fn_tagged, arg_tagged]);
                    let result = b.inst_results(call)[0];
                    current_depth -= 1;
                    b.def_var(vars[current_depth - 1], result);
                    var_is_raw[current_depth - 1] = false;
                    slot_types[current_depth - 1] = JitType::Unknown;
                } else {
                    bail!();
                }
            }

            // Arithmetic
            OpCode::Add | OpCode::Subtract | OpCode::Multiply | OpCode::Divide => {
                if current_depth < 2 { bail!(); }
                let a  = current_depth - 2;
                let b_ = current_depth - 1;
                let both_proven_float = slot_types[a] == JitType::ProvenFloat
                                     && slot_types[b_] == JitType::ProvenFloat;
                let both_proven_int = slot_types[a] == JitType::ProvenInt
                                   && slot_types[b_] == JitType::ProvenInt;
                let any_unknown = slot_types[a] == JitType::Unknown
                               || slot_types[b_] == JitType::Unknown;

                let (result, result_type) = if both_proven_float {
                    let av_i = b.use_var(vars[a]);
                    let bv_i = b.use_var(vars[b_]);
                    let av_f = b.ins().bitcast(types::F64, MemFlags::new(), av_i);
                    let bv_f = b.ins().bitcast(types::F64, MemFlags::new(), bv_i);
                    let fr = match op {
                        OpCode::Add      => b.ins().fadd(av_f, bv_f),
                        OpCode::Subtract => b.ins().fsub(av_f, bv_f),
                        OpCode::Multiply => b.ins().fmul(av_f, bv_f),
                        OpCode::Divide   => b.ins().fdiv(av_f, bv_f),
                        _ => unreachable!(),
                    };
                    (b.ins().bitcast(types::I64, MemFlags::new(), fr), JitType::ProvenFloat)
                } else if both_proven_int {
                    let av = get_raw!(a);
                    let bv = get_raw!(b_);
                    let r = match op {
                        OpCode::Add      => b.ins().iadd(av, bv),
                        OpCode::Subtract => b.ins().isub(av, bv),
                        OpCode::Multiply => b.ins().imul(av, bv),
                        OpCode::Divide   => b.ins().sdiv(av, bv),
                        _ => unreachable!(),
                    };
                    let c_max = b.ins().iconst(types::I64, 0x00007FFFFFFFFFFFi64);
                    let c_min = b.ins().iconst(types::I64, -0x0000800000000000i64);
                    let ovf = b.ins().icmp(IntCC::SignedGreaterThan, r, c_max);
                    let unf = b.ins().icmp(IntCC::SignedLessThan, r, c_min);
                    let out_of_bounds = b.ins().bor(ovf, unf);
                    let ok_blk = b.create_block();
                    let err_blk = b.create_block();
                    b.ins().brif(out_of_bounds, err_blk, &[], ok_blk, &[]);
                    
                    b.switch_to_block(err_blk);
                    b.seal_block(err_blk);
                    deopt_ret!(ip);
                    
                    b.switch_to_block(ok_blk);
                    b.seal_block(ok_blk);
                    (r, JitType::ProvenInt)
                } else if !any_unknown {
                    // Static mixed: one ProvenFloat + one ProvenInt
                    let av_i = b.use_var(vars[a]);
                    let bv_i = b.use_var(vars[b_]);
                    let av_f = if slot_types[a] == JitType::ProvenFloat {
                        b.ins().bitcast(types::F64, MemFlags::new(), av_i)
                    } else {
                        let raw = get_raw!(a);
                        b.ins().fcvt_from_sint(types::F64, raw)
                    };
                    let bv_f = if slot_types[b_] == JitType::ProvenFloat {
                        b.ins().bitcast(types::F64, MemFlags::new(), bv_i)
                    } else {
                        let raw = get_raw!(b_);
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
                } else {
                    // ── Runtime dispatch: at least one Unknown operand ────
                    let av = b.use_var(vars[a]);
                    let bv = b.use_var(vars[b_]);
                    let c_qnan_val = b.ins().iconst(types::I64, qnan as i64);

                    let a_and = b.ins().band(av, c_qnan_val);
                    let a_is_float = b.ins().icmp(IntCC::NotEqual, a_and, c_qnan_val);
                    let b_and = b.ins().band(bv, c_qnan_val);
                    let b_is_float = b.ins().icmp(IntCC::NotEqual, b_and, c_qnan_val);
                    let either_float = b.ins().bor(a_is_float, b_is_float);

                    let c_obj_mask = b.ins().iconst(types::I64, (crate::value::SIGN_BIT | crate::value::QNAN) as i64);
                    let a_obj_bits = b.ins().band(av, c_obj_mask);
                    let a_is_obj   = b.ins().icmp(IntCC::Equal, a_obj_bits, c_obj_mask);
                    let b_obj_bits = b.ins().band(bv, c_obj_mask);
                    let b_is_obj   = b.ins().icmp(IntCC::Equal, b_obj_bits, c_obj_mask);
                    let any_obj    = b.ins().bor(a_is_obj, b_is_obj);
                    let obj_check_blk = b.create_block();
                    let num_check_blk = b.create_block();
                    b.ins().brif(any_obj, obj_check_blk, &[], num_check_blk, &[]);
                    b.switch_to_block(obj_check_blk); deopt_ret!(ip);
                    b.seal_block(obj_check_blk);
                    b.switch_to_block(num_check_blk);
                    b.seal_block(num_check_blk);

                    let float_blk = b.create_block();
                    let int_blk = b.create_block();
                    let merge_blk = b.create_block();
                    b.append_block_param(merge_blk, types::I64);

                    b.ins().brif(either_float, float_blk, &[], int_blk, &[]);

                    // ── Float path ──
                    b.switch_to_block(float_blk);
                    b.seal_block(float_blk);
                    let make_f64 = |builder: &mut FunctionBuilder, sv: cranelift::prelude::Value,
                                    st: JitType, is_raw: bool| -> cranelift::prelude::Value {
                        if st == JitType::ProvenFloat {
                            builder.ins().bitcast(types::F64, MemFlags::new(), sv)
                        } else if st == JitType::ProvenInt {
                            let raw = if is_raw { sv } else {
                                let masked = builder.ins().band(sv, c_pmask);
                                let amt = builder.ins().iconst(types::I32, 16);
                                let shl = builder.ins().ishl(masked, amt);
                                builder.ins().sshr(shl, amt)
                            };
                            builder.ins().fcvt_from_sint(types::F64, raw)
                        } else {
                            let sv_and = builder.ins().band(sv, c_qnan_val);
                            let sv_is_float = builder.ins().icmp(IntCC::NotEqual, sv_and, c_qnan_val);
                            let as_float = builder.ins().bitcast(types::F64, MemFlags::new(), sv);
                            let raw_payload = builder.ins().band(sv, c_pmask);
                            let amt = builder.ins().iconst(types::I32, 16);
                            let shl = builder.ins().ishl(raw_payload, amt);
                            let sext = builder.ins().sshr(shl, amt);
                            let as_int_f = builder.ins().fcvt_from_sint(types::F64, sext);
                            builder.ins().select(sv_is_float, as_float, as_int_f)
                        }
                    };
                    let af = make_f64(&mut b, av, slot_types[a], var_is_raw[a]);
                    let bf = make_f64(&mut b, bv, slot_types[b_], var_is_raw[b_]);
                    let fr = match op {
                        OpCode::Add      => b.ins().fadd(af, bf),
                        OpCode::Subtract => b.ins().fsub(af, bf),
                        OpCode::Multiply => b.ins().fmul(af, bf),
                        OpCode::Divide   => b.ins().fdiv(af, bf),
                        _ => unreachable!(),
                    };
                    let float_res = b.ins().bitcast(types::I64, MemFlags::new(), fr);
                    b.ins().jump(merge_blk, &[float_res]);

                    // ── Int path ──
                    b.switch_to_block(int_blk);
                    b.seal_block(int_blk);
                    let a_raw = get_raw!(a);
                    let b_raw = get_raw!(b_);
                    let ir = match op {
                        OpCode::Add      => b.ins().iadd(a_raw, b_raw),
                        OpCode::Subtract => b.ins().isub(a_raw, b_raw),
                        OpCode::Multiply => b.ins().imul(a_raw, b_raw),
                        OpCode::Divide   => b.ins().sdiv(a_raw, b_raw),
                        _ => unreachable!(),
                    };
                    let c_max = b.ins().iconst(types::I64, 0x00007FFFFFFFFFFFi64);
                    let c_min = b.ins().iconst(types::I64, -0x0000800000000000i64);
                    let ovf = b.ins().icmp(IntCC::SignedGreaterThan, ir, c_max);
                    let unf = b.ins().icmp(IntCC::SignedLessThan, ir, c_min);
                    let out_of_bounds = b.ins().bor(ovf, unf);
                    let ok_blk = b.create_block();
                    let err_blk = b.create_block();
                    b.ins().brif(out_of_bounds, err_blk, &[], ok_blk, &[]);
                    
                    b.switch_to_block(err_blk);
                    b.seal_block(err_blk);
                    deopt_ret!(ip);
                    
                    b.switch_to_block(ok_blk);
                    b.seal_block(ok_blk);
                    
                    let ir_masked = b.ins().band(ir, c_pmask);
                    let int_res = b.ins().bor(ir_masked, c_prefix);
                    b.ins().jump(merge_blk, &[int_res]);

                    // ── Merge ──
                    b.switch_to_block(merge_blk);
                    b.seal_block(merge_blk);
                    let merged = b.block_params(merge_blk)[0];
                    (merged, JitType::Unknown)
                };
                current_depth -= 1;
                let is_raw = result_type == JitType::ProvenInt || result_type == JitType::ProvenFloat;
                b.def_var(vars[current_depth - 1], result);
                var_is_raw[current_depth - 1] = is_raw;
                slot_types[current_depth - 1] = result_type;
            }

            OpCode::Equal | OpCode::Greater | OpCode::Less => {
                if current_depth < 2 { bail!(); }
                let a  = current_depth - 2;
                let bx = current_depth - 1;
                let both_proven_float = slot_types[a] == JitType::ProvenFloat
                                     && slot_types[bx] == JitType::ProvenFloat;
                let both_proven_int = slot_types[a] == JitType::ProvenInt
                                   && slot_types[bx] == JitType::ProvenInt;
                let any_unknown = slot_types[a] == JitType::Unknown
                               || slot_types[bx] == JitType::Unknown;

                let cond = if both_proven_float {
                    let av_i = b.use_var(vars[a]);
                    let bv_i = b.use_var(vars[bx]);
                    let av_f = b.ins().bitcast(types::F64, MemFlags::new(), av_i);
                    let bv_f = b.ins().bitcast(types::F64, MemFlags::new(), bv_i);
                    let fcc = if matches!(op, OpCode::Equal) { FloatCC::Equal }
                              else if matches!(op, OpCode::Greater) { FloatCC::GreaterThan }
                              else { FloatCC::LessThan };
                    b.ins().fcmp(fcc, av_f, bv_f)
                } else if both_proven_int {
                    let av = get_raw!(a); let bv = get_raw!(bx);
                    let cc = if matches!(op, OpCode::Equal) { IntCC::Equal }
                             else if matches!(op, OpCode::Greater) { IntCC::SignedGreaterThan }
                             else { IntCC::SignedLessThan };
                    b.ins().icmp(cc, av, bv)
                } else if matches!(op, OpCode::Equal) && !any_unknown {
                    let av = get_tagged!(a); let bv = get_tagged!(bx);
                    b.ins().icmp(IntCC::Equal, av, bv)
                } else if any_unknown {
                    // Runtime dispatch for Unknown comparison operands
                    let av = b.use_var(vars[a]);
                    let bv = b.use_var(vars[bx]);
                    let c_qnan_val = b.ins().iconst(types::I64, qnan as i64);
                    let a_and = b.ins().band(av, c_qnan_val);
                    let a_is_float = b.ins().icmp(IntCC::NotEqual, a_and, c_qnan_val);
                    let b_and = b.ins().band(bv, c_qnan_val);
                    let b_is_float = b.ins().icmp(IntCC::NotEqual, b_and, c_qnan_val);
                    let either_float = b.ins().bor(a_is_float, b_is_float);

                    let float_cmp_blk = b.create_block();
                    let int_cmp_blk = b.create_block();
                    let cmp_merge_blk = b.create_block();
                    b.append_block_param(cmp_merge_blk, types::I8);

                    b.ins().brif(either_float, float_cmp_blk, &[], int_cmp_blk, &[]);

                    // ── Float path ──
                    b.switch_to_block(float_cmp_blk);
                    b.seal_block(float_cmp_blk);
                    let make_f64 = |builder: &mut FunctionBuilder, sv: cranelift::prelude::Value,
                                    st: JitType, is_raw: bool| -> cranelift::prelude::Value {
                        if st == JitType::ProvenFloat {
                            builder.ins().bitcast(types::F64, MemFlags::new(), sv)
                        } else if st == JitType::ProvenInt {
                            let raw = if is_raw { sv } else {
                                let masked = builder.ins().band(sv, c_pmask);
                                let amt = builder.ins().iconst(types::I32, 16);
                                let shl = builder.ins().ishl(masked, amt);
                                builder.ins().sshr(shl, amt)
                            };
                            builder.ins().fcvt_from_sint(types::F64, raw)
                        } else {
                            let sv_and = builder.ins().band(sv, c_qnan_val);
                            let sv_is_float = builder.ins().icmp(IntCC::NotEqual, sv_and, c_qnan_val);
                            let as_float = builder.ins().bitcast(types::F64, MemFlags::new(), sv);
                            let raw_payload = builder.ins().band(sv, c_pmask);
                            let amt = builder.ins().iconst(types::I32, 16);
                            let shl = builder.ins().ishl(raw_payload, amt);
                            let sext = builder.ins().sshr(shl, amt);
                            let as_int_f = builder.ins().fcvt_from_sint(types::F64, sext);
                            builder.ins().select(sv_is_float, as_float, as_int_f)
                        }
                    };
                    let af = make_f64(&mut b, av, slot_types[a], var_is_raw[a]);
                    let bf = make_f64(&mut b, bv, slot_types[bx], var_is_raw[bx]);
                    let fcc = if matches!(op, OpCode::Equal) { FloatCC::Equal }
                              else if matches!(op, OpCode::Greater) { FloatCC::GreaterThan }
                              else { FloatCC::LessThan };
                    let fcond = b.ins().fcmp(fcc, af, bf);
                    b.ins().jump(cmp_merge_blk, &[fcond]);

                    // ── Int path ──
                    b.switch_to_block(int_cmp_blk);
                    b.seal_block(int_cmp_blk);
                    let a_raw = if var_is_raw[a] { av } else { b.ins().band(av, c_pmask) };
                    let b_raw = if var_is_raw[bx] { bv } else { b.ins().band(bv, c_pmask) };
                    let icc = if matches!(op, OpCode::Equal) { IntCC::Equal }
                              else if matches!(op, OpCode::Greater) { IntCC::SignedGreaterThan }
                              else { IntCC::SignedLessThan };
                    let icond = b.ins().icmp(icc, a_raw, b_raw);
                    b.ins().jump(cmp_merge_blk, &[icond]);

                    b.switch_to_block(cmp_merge_blk);
                    b.seal_block(cmp_merge_blk);
                    b.block_params(cmp_merge_blk)[0]
                } else {
                    ensure_raw_int!(a,  ip);
                    ensure_raw_int!(bx, ip);
                    let cc = if matches!(op, OpCode::Greater) { IntCC::SignedGreaterThan }
                             else { IntCC::SignedLessThan };
                    let av = get_raw!(a); let bv = get_raw!(bx); b.ins().icmp(cc, av, bv)
                };
                last_cmp = Some(cond);
                is_cmp   = true;
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
                b.ins().jump(ip_to_block[&target_ip], &args);
                block_terminated = true;
            }

            OpCode::Return => {
                let rv = if current_depth > 0 {
                    let s = current_depth - 1;
                    if slot_types[s] == JitType::ProvenFloat {
                        b.use_var(vars[s])
                    } else if var_is_raw[s] {
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

            _ => bail!(), // unsupported opcode — fall back to interpreter
        }

        if !is_cmp { last_cmp = None; }
    }

    if !block_terminated { b.ins().return_(&[c_null]); }

    // Seal all bytecode blocks
    for (_, &blk) in &ip_to_block { b.seal_block(blk); }
    b.finalize();

    let name = format!("quin_{}_{}", function.name, engine.fn_counter);
    engine.fn_counter += 1;

    let id = match engine.module.declare_function(&name, Linkage::Export, &engine.ctx.func.signature) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[JIT] declare_function failed for {}: {:?}", function.name, e);
            return std::ptr::null();
        }
    };
    if let Err(e) = engine.module.define_function(id, &mut engine.ctx) {
        eprintln!("[JIT] define_function failed for {}: {:?}", function.name, e);
        return std::ptr::null();
    }
    if let Err(e) = engine.module.finalize_definitions() {
        eprintln!("[JIT] finalize_definitions failed for {}: {:?}", function.name, e);
        return std::ptr::null();
    }

    engine.module.get_finalized_function(id)
}
