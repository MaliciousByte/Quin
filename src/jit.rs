use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use crate::value::Function;
use std::collections::HashMap;

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
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();
        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        let module = JITModule::new(builder);
        Self {
            ctx: codegen::Context::new(),
            module,
            fn_counter: 0,
        }
    }

    pub fn compile(&mut self, function: &Function) -> *const u8 {
        self.module.clear_context(&mut self.ctx);

        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // *mut VM
        sig.params.push(AbiParam::new(self.module.target_config().pointer_type())); // *const u64 (args)
        sig.returns.push(AbiParam::new(types::I64)); // Returns Value
        self.ctx.func.signature = sig;

        let mut builder_context = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut builder_context);

        // --- NaN boxing constants ---
        let qnan: u64 = 0x7ff8000000000000;
        let tag_int: u64 = 0x0004000000000000;
        let tag_deopt: u64 = 0x0007000000000000;
        let tag_null: u64 = 0x0001000000000000;
        let tag_false: u64 = 0x0002000000000000;
        let tag_true: u64 = 0x0003000000000000;
        let int_mask: u64 = 0xFFFF000000000000;
        let int_prefix: u64 = qnan | tag_int;
        let payload_mask_val: u64 = 0x0000FFFFFFFFFFFF;
        let null_val_raw: i64 = (qnan | tag_null) as i64;
        let false_val_raw: i64 = (qnan | tag_false) as i64;
        let true_val_raw: i64 = (qnan | tag_true) as i64;

        let chunk = &function.chunk;

        // =====================================================================
        // PASS 1: Discover all branch targets and create Cranelift blocks
        // =====================================================================
        // Every IP that is a jump target needs its own block.
        // IP 0 is always the entry block.
        let mut block_starts: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        block_starts.insert(0);

        for ip in 0..chunk.code.len() {
            match &chunk.code[ip] {
                crate::chunk::OpCode::JumpIfFalse(offset) => {
                    // Fall-through is ip+1, target is ip+1+offset
                    block_starts.insert(ip + 1);
                    block_starts.insert(ip + 1 + offset);
                }
                crate::chunk::OpCode::Jump(offset) => {
                    block_starts.insert(ip + 1 + offset);
                    // ip+1 is dead unless something jumps to it, but we still
                    // create a block so Cranelift doesn't see instructions after
                    // a terminator.
                    if ip + 1 < chunk.code.len() {
                        block_starts.insert(ip + 1);
                    }
                }
                crate::chunk::OpCode::Loop(offset) => {
                    // Backward jump: target is ip+1-offset
                    let target = (ip + 1).wrapping_sub(*offset);
                    block_starts.insert(target);
                    if ip + 1 < chunk.code.len() {
                        block_starts.insert(ip + 1);
                    }
                }
                crate::chunk::OpCode::Return => {
                    if ip + 1 < chunk.code.len() {
                        block_starts.insert(ip + 1);
                    }
                }
                _ => {}
            }
        }

        // Create Cranelift blocks for each discovered target IP
        let mut ip_to_block: HashMap<usize, Block> = HashMap::new();
        for &ip in &block_starts {
            let block = builder.create_block();
            ip_to_block.insert(ip, block);
        }

        // The entry block (IP 0) gets the function params
        let entry_block = ip_to_block[&0];
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);

        let args_ptr = builder.block_params(entry_block)[1];

        // =====================================================================
        // Virtual stack — tracks Cranelift SSA values per bytecode position
        // =====================================================================
        let mut vstack: Vec<codegen::ir::Value> = Vec::new();

        // Load function arguments onto the virtual stack
        for i in 0..function.arity {
            let offset = (i * 8) as i32;
            let val = builder.ins().load(types::I64, MemFlags::new(), args_ptr, offset);
            vstack.push(val);
        }

        // Track whether the current block has been terminated
        let mut block_terminated = false;

        // =====================================================================
        // PASS 2: Emit Cranelift IR for each opcode
        // =====================================================================
        for ip in 0..chunk.code.len() {
            // If this IP starts a new block, switch to it
            if ip > 0 {
                if let Some(&target_block) = ip_to_block.get(&ip) {
                    // If the previous block wasn't terminated, fall through
                    if !block_terminated {
                        builder.ins().jump(target_block, &[]);
                    }
                    builder.switch_to_block(target_block);
                    block_terminated = false;
                }
            }

            // If we're in a dead block (after return/jump), skip until next block start
            if block_terminated {
                continue;
            }

            let op = &chunk.code[ip];
            match op {
                // ----- Constants -----
                crate::chunk::OpCode::Null => {
                    let v = builder.ins().iconst(types::I64, null_val_raw);
                    vstack.push(v);
                }
                crate::chunk::OpCode::True => {
                    let v = builder.ins().iconst(types::I64, true_val_raw);
                    vstack.push(v);
                }
                crate::chunk::OpCode::False => {
                    let v = builder.ins().iconst(types::I64, false_val_raw);
                    vstack.push(v);
                }
                crate::chunk::OpCode::Constant(idx) => {
                    let val = chunk.constants[*idx].0;
                    let iconst = builder.ins().iconst(types::I64, val as i64);
                    vstack.push(iconst);
                }

                // ----- Stack management -----
                crate::chunk::OpCode::Pop => {
                    if vstack.pop().is_none() {
                        return std::ptr::null();
                    }
                }
                crate::chunk::OpCode::Dup => {
                    if let Some(&top) = vstack.last() {
                        vstack.push(top);
                    } else {
                        return std::ptr::null();
                    }
                }

                // ----- Locals -----
                crate::chunk::OpCode::GetLocal(idx) => {
                    let idx_usize = *idx;
                    if idx_usize < vstack.len() {
                        let val = vstack[idx_usize];
                        vstack.push(val);
                    } else {
                        return std::ptr::null();
                    }
                }
                crate::chunk::OpCode::SetLocal(idx) => {
                    let idx_usize = *idx;
                    if let Some(&top) = vstack.last() {
                        if idx_usize < vstack.len() {
                            vstack[idx_usize] = top;
                        } else {
                            return std::ptr::null();
                        }
                    } else {
                        return std::ptr::null();
                    }
                }

                // ----- Arithmetic (int fast path with deopt) -----
                crate::chunk::OpCode::Add
                | crate::chunk::OpCode::Subtract
                | crate::chunk::OpCode::Multiply
                | crate::chunk::OpCode::Divide => {
                    if vstack.len() < 2 {
                        return std::ptr::null();
                    }
                    let b = vstack.pop().unwrap();
                    let a = vstack.pop().unwrap();

                    let mask = builder.ins().iconst(types::I64, int_mask as i64);
                    let prefix = builder.ins().iconst(types::I64, int_prefix as i64);

                    let a_tag = builder.ins().band(a, mask);
                    let b_tag = builder.ins().band(b, mask);

                    let a_is_int = builder.ins().icmp(IntCC::Equal, a_tag, prefix);
                    let b_is_int = builder.ins().icmp(IntCC::Equal, b_tag, prefix);
                    let both_int = builder.ins().band(a_is_int, b_is_int);

                    let deopt_block = builder.create_block();
                    let next_block = builder.create_block();

                    builder.ins().brif(both_int, next_block, &[], deopt_block, &[]);

                    builder.switch_to_block(deopt_block);
                    let deopt_val = (qnan | tag_deopt | (ip as u64 & payload_mask_val)) as i64;
                    let deopt_ret = builder.ins().iconst(types::I64, deopt_val);
                    builder.ins().return_(&[deopt_ret]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(next_block);
                    builder.seal_block(next_block);

                    let p_mask = builder.ins().iconst(types::I64, payload_mask_val as i64);
                    let a_raw = builder.ins().band(a, p_mask);
                    let b_raw = builder.ins().band(b, p_mask);

                    let res_raw = match op {
                        crate::chunk::OpCode::Add => builder.ins().iadd(a_raw, b_raw),
                        crate::chunk::OpCode::Subtract => builder.ins().isub(a_raw, b_raw),
                        crate::chunk::OpCode::Multiply => builder.ins().imul(a_raw, b_raw),
                        crate::chunk::OpCode::Divide => builder.ins().sdiv(a_raw, b_raw),
                        _ => unreachable!(),
                    };

                    let res = builder.ins().bor(res_raw, prefix);
                    vstack.push(res);
                }

                // ----- Comparisons (int fast path with deopt) -----
                crate::chunk::OpCode::Equal => {
                    if vstack.len() < 2 {
                        return std::ptr::null();
                    }
                    let b = vstack.pop().unwrap();
                    let a = vstack.pop().unwrap();

                    // Raw u64 equality works for all NaN-boxed non-float types
                    let is_eq = builder.ins().icmp(IntCC::Equal, a, b);
                    let true_c = builder.ins().iconst(types::I64, true_val_raw);
                    let false_c = builder.ins().iconst(types::I64, false_val_raw);
                    let result = builder.ins().select(is_eq, true_c, false_c);
                    vstack.push(result);
                }
                crate::chunk::OpCode::Greater | crate::chunk::OpCode::Less => {
                    if vstack.len() < 2 {
                        return std::ptr::null();
                    }
                    let b = vstack.pop().unwrap();
                    let a = vstack.pop().unwrap();

                    // Type check: both must be int
                    let mask = builder.ins().iconst(types::I64, int_mask as i64);
                    let prefix = builder.ins().iconst(types::I64, int_prefix as i64);
                    let a_tag = builder.ins().band(a, mask);
                    let b_tag = builder.ins().band(b, mask);
                    let a_is_int = builder.ins().icmp(IntCC::Equal, a_tag, prefix);
                    let b_is_int = builder.ins().icmp(IntCC::Equal, b_tag, prefix);
                    let both_int = builder.ins().band(a_is_int, b_is_int);

                    let deopt_block = builder.create_block();
                    let cmp_block = builder.create_block();

                    builder.ins().brif(both_int, cmp_block, &[], deopt_block, &[]);

                    builder.switch_to_block(deopt_block);
                    let deopt_val = (qnan | tag_deopt | (ip as u64 & payload_mask_val)) as i64;
                    let deopt_ret = builder.ins().iconst(types::I64, deopt_val);
                    builder.ins().return_(&[deopt_ret]);
                    builder.seal_block(deopt_block);

                    builder.switch_to_block(cmp_block);
                    builder.seal_block(cmp_block);

                    // Extract signed payloads and compare
                    let p_mask = builder.ins().iconst(types::I64, payload_mask_val as i64);
                    let a_raw = builder.ins().band(a, p_mask);
                    let b_raw = builder.ins().band(b, p_mask);

                    // Sign extend from 48 bits for signed comparison
                    let shift_amt = builder.ins().iconst(types::I64, 16);
                    let a_shifted = builder.ins().ishl(a_raw, shift_amt);
                    let a_signed = builder.ins().sshr(a_shifted, shift_amt);
                    let b_shifted = builder.ins().ishl(b_raw, shift_amt);
                    let b_signed = builder.ins().sshr(b_shifted, shift_amt);

                    let cc = if matches!(op, crate::chunk::OpCode::Greater) {
                        IntCC::SignedGreaterThan
                    } else {
                        IntCC::SignedLessThan
                    };

                    let cmp_result = builder.ins().icmp(cc, a_signed, b_signed);
                    let true_c = builder.ins().iconst(types::I64, true_val_raw);
                    let false_c = builder.ins().iconst(types::I64, false_val_raw);
                    let result = builder.ins().select(cmp_result, true_c, false_c);
                    vstack.push(result);
                }

                // ----- Logical / Unary -----
                crate::chunk::OpCode::Not => {
                    if let Some(val) = vstack.pop() {
                        // Falsey: null or false → return true, else false
                        let null_c = builder.ins().iconst(types::I64, null_val_raw);
                        let false_c = builder.ins().iconst(types::I64, false_val_raw);
                        let true_c = builder.ins().iconst(types::I64, true_val_raw);

                        let is_null = builder.ins().icmp(IntCC::Equal, val, null_c);
                        let is_false = builder.ins().icmp(IntCC::Equal, val, false_c);
                        let is_falsey = builder.ins().bor(is_null, is_false);
                        let result = builder.ins().select(is_falsey, true_c, false_c);
                        vstack.push(result);
                    } else {
                        return std::ptr::null();
                    }
                }
                crate::chunk::OpCode::Negate => {
                    if let Some(val) = vstack.pop() {
                        // Type check: must be int
                        let mask = builder.ins().iconst(types::I64, int_mask as i64);
                        let prefix = builder.ins().iconst(types::I64, int_prefix as i64);
                        let val_tag = builder.ins().band(val, mask);
                        let is_int = builder.ins().icmp(IntCC::Equal, val_tag, prefix);

                        let deopt_block = builder.create_block();
                        let neg_block = builder.create_block();

                        builder.ins().brif(is_int, neg_block, &[], deopt_block, &[]);

                        builder.switch_to_block(deopt_block);
                        let deopt_val = (qnan | tag_deopt | (ip as u64 & payload_mask_val)) as i64;
                        let deopt_ret = builder.ins().iconst(types::I64, deopt_val);
                        builder.ins().return_(&[deopt_ret]);
                        builder.seal_block(deopt_block);

                        builder.switch_to_block(neg_block);
                        builder.seal_block(neg_block);

                        let p_mask = builder.ins().iconst(types::I64, payload_mask_val as i64);
                        let raw = builder.ins().band(val, p_mask);
                        let negated = builder.ins().ineg(raw);
                        // Mask to 48 bits and re-tag
                        let masked = builder.ins().band(negated, p_mask);
                        let result = builder.ins().bor(masked, prefix);
                        vstack.push(result);
                    } else {
                        return std::ptr::null();
                    }
                }

                // ----- Control flow -----
                crate::chunk::OpCode::JumpIfFalse(offset) => {
                    if let Some(&val) = vstack.last() {
                        // Falsey check: null or false
                        let null_c = builder.ins().iconst(types::I64, null_val_raw);
                        let false_c = builder.ins().iconst(types::I64, false_val_raw);
                        let is_null = builder.ins().icmp(IntCC::Equal, val, null_c);
                        let is_false = builder.ins().icmp(IntCC::Equal, val, false_c);
                        let is_falsey = builder.ins().bor(is_null, is_false);

                        let target_ip = ip + 1 + offset;
                        let fall_ip = ip + 1;

                        let target_block = match ip_to_block.get(&target_ip) {
                            Some(&b) => b,
                            None => return std::ptr::null(),
                        };
                        let fall_block = match ip_to_block.get(&fall_ip) {
                            Some(&b) => b,
                            None => return std::ptr::null(),
                        };

                        builder.ins().brif(is_falsey, target_block, &[], fall_block, &[]);
                        block_terminated = true;
                    } else {
                        return std::ptr::null();
                    }
                }
                crate::chunk::OpCode::Jump(offset) => {
                    let target_ip = ip + 1 + offset;
                    let target_block = match ip_to_block.get(&target_ip) {
                        Some(&b) => b,
                        None => return std::ptr::null(),
                    };
                    builder.ins().jump(target_block, &[]);
                    block_terminated = true;
                }
                crate::chunk::OpCode::Loop(offset) => {
                    let target_ip = (ip + 1).wrapping_sub(*offset);
                    let target_block = match ip_to_block.get(&target_ip) {
                        Some(&b) => b,
                        None => return std::ptr::null(),
                    };
                    builder.ins().jump(target_block, &[]);
                    block_terminated = true;
                }

                // ----- Return -----
                crate::chunk::OpCode::Return => {
                    if let Some(ret_val) = vstack.pop() {
                        builder.ins().return_(&[ret_val]);
                    } else {
                        let null_ret = builder.ins().iconst(types::I64, null_val_raw);
                        builder.ins().return_(&[null_ret]);
                    }
                    block_terminated = true;
                }

                // ----- Unsupported → bail -----
                _ => {
                    return std::ptr::null();
                }
            }
        }

        // If the last block wasn't terminated, return null
        if !block_terminated {
            let null_ret = builder.ins().iconst(types::I64, null_val_raw);
            builder.ins().return_(&[null_ret]);
        }

        // Seal all blocks — we've emitted all branches
        for (_, &block) in &ip_to_block {
            builder.seal_block(block);
        }

        builder.finalize();

        // Use a unique name per compilation to avoid Cranelift symbol collisions
        let unique_name = format!("{}_{}", function.name, self.fn_counter);
        self.fn_counter += 1;

        let id = self.module
            .declare_function(&unique_name, Linkage::Export, &self.ctx.func.signature)
            .unwrap();
        self.module.define_function(id, &mut self.ctx).unwrap();
        self.module.finalize_definitions().unwrap();

        self.module.get_finalized_function(id)
    }
}
