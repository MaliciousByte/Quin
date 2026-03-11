use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use crate::value::Function;

pub struct JitEngine {
    ctx: codegen::Context,
    module: JITModule,
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
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let args_ptr = builder.block_params(entry_block)[1];
        let mut vstack: Vec<codegen::ir::Value> = Vec::new();
        
        for i in 0..function.arity {
            let offset = (i * 8) as i32;
            let val = builder.ins().load(types::I64, MemFlags::new(), args_ptr, offset);
            vstack.push(val);
        }
        
        let chunk = &function.chunk;
        let mut ip = 0;
        
        let qnan = 0x7ff8000000000000u64;
        let tag_int = 0x0004000000000000u64;
        let tag_deopt = 0x0007000000000000u64;
        let int_mask = 0xFFFF000000000000u64;
        let int_prefix = qnan | tag_int;
        let payload_mask_val = 0x0000FFFFFFFFFFFFu64;
        let null_val_raw = (qnan | 0x0001000000000000u64) as i64;

        while ip < chunk.code.len() {
            let op = &chunk.code[ip];
            match op {
                crate::chunk::OpCode::Null => {
                    let null_val = builder.ins().iconst(types::I64, null_val_raw);
                    vstack.push(null_val);
                }
                crate::chunk::OpCode::Constant(idx) => {
                    let val = chunk.constants[*idx as usize].0;
                    let iconst = builder.ins().iconst(types::I64, val as i64);
                    vstack.push(iconst);
                }
                crate::chunk::OpCode::Add | crate::chunk::OpCode::Subtract => {
                    if vstack.len() >= 2 {
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
                        let deopt_val = (qnan | tag_deopt | (ip as u64 & 0x0000FFFFFFFFFFFF)) as i64;
                        let deopt_ret = builder.ins().iconst(types::I64, deopt_val);
                        builder.ins().return_(&[deopt_ret]); 
                        builder.seal_block(deopt_block);
                        
                        builder.switch_to_block(next_block);
                        builder.seal_block(next_block);

                        let p_mask = builder.ins().iconst(types::I64, payload_mask_val as i64);
                        let a_raw = builder.ins().band(a, p_mask);
                        let b_raw = builder.ins().band(b, p_mask);
                        
                        let res_raw = if matches!(op, crate::chunk::OpCode::Add) {
                            builder.ins().iadd(a_raw, b_raw)
                        } else {
                            builder.ins().isub(a_raw, b_raw)
                        };
                        
                        let res = builder.ins().bor(res_raw, prefix);
                        vstack.push(res);
                    }
                }
                crate::chunk::OpCode::GetLocal(idx) => {
                    let idx_usize = *idx as usize;
                    if idx_usize < function.arity {
                        let offset = (idx_usize * 8) as i32;
                        let val = builder.ins().load(types::I64, MemFlags::new(), args_ptr, offset);
                        vstack.push(val);
                    } else {
                        // Bail during compilation for non-argument locals
                        return std::ptr::null();
                    }
                }
                crate::chunk::OpCode::Return => {
                    if let Some(ret_val) = vstack.pop() {
                        builder.ins().return_(&[ret_val]);
                    } else {
                        let null_ret = builder.ins().iconst(types::I64, null_val_raw);
                        builder.ins().return_(&[null_ret]);
                    }
                    
                    // After a return, the rest of the block is dead.
                    // Start a new block to keep Cranelift happy during subsequent opcode translation.
                    if ip + 1 < chunk.code.len() {
                        let dead_block = builder.create_block();
                        builder.switch_to_block(dead_block);
                        builder.seal_block(dead_block);
                    }
                }
                _ => {
                    // Bail during compilation for unsupported opcodes
                    return std::ptr::null();
                }
            }
            ip += 1;
        }

        builder.finalize();

        let id = self.module.declare_function(&function.name, Linkage::Export, &self.ctx.func.signature).unwrap();
        self.module.define_function(id, &mut self.ctx).unwrap();
        self.module.finalize_definitions().unwrap();
        
        let code = self.module.get_finalized_function(id);
        code
    }
}
