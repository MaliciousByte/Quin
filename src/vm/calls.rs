use std::sync::Arc;
use std::cell::RefCell;
use crate::value::{Value, Closure, InstanceValue};
use crate::vm::obj::Obj;
use super::{VM, CallFrame};

impl VM {
    pub fn call_value_native(&mut self, callee: Value, args: &[Value]) -> Result<(), String> {
        let caller_offset = if let Ok(f) = self.current_frame() { f.stack_offset } else { 0 };
        let callee_reg = (self.stack.len() - caller_offset) as u8;
        self.push(callee);
        for arg in args {
            self.push(arg.clone());
        }
        self.call_value(callee_reg, args.len() as u8, None)
    }

    pub fn call_value(&mut self, callee_reg: u8, arg_count: u8, dst: Option<u8>) -> Result<(), String> {
        let caller_offset = if let Ok(f) = self.current_frame() { f.stack_offset } else { 0 };
        let callee = self.stack[caller_offset + callee_reg as usize].clone();
        if !callee.is_obj() {
            return Err(format!("Can only call functions, closures, and classes. Got: {}", callee));
        }

        match &*callee.as_obj() {
            Obj::Function(fun) => {
                let closure = Arc::new(Closure {
                    function: fun.clone(),
                    upvalues: Vec::new(),
                });
                self.call_closure(closure, arg_count, callee_reg, dst)
            }
            Obj::BoundMethod(bm) => {
                let callee_stack_idx = caller_offset + callee_reg as usize;
                self.stack[callee_stack_idx] = bm.receiver.clone();
                let closure = Arc::new(Closure {
                    function: bm.method.clone(),
                    upvalues: Vec::new(),
                });
                self.call_closure(closure, arg_count, callee_reg, dst)
            }
            Obj::Closure(closure) => self.call_closure(closure.clone(), arg_count, callee_reg, dst),
            Obj::Class(cls) => {
                let inst = Arc::new(RefCell::new(InstanceValue {
                    class: cls.clone(),
                    shape: self.root_shape.clone(),
                    fields: RefCell::new(Vec::new()),
                }));
                let inst_val = Value::obj(Arc::new(Obj::Object(inst.clone())));
                
                let callee_stack_idx = caller_offset + callee_reg as usize;
                
                // Write self to the callee slot as receiver for constructor (or direct return)
                if self.stack.len() <= callee_stack_idx {
                    self.stack.resize(callee_stack_idx + 1, Value::null());
                }
                self.stack[callee_stack_idx] = inst_val.clone();
                
                let mut constructor_val = None;
                let mut current_class = Some(cls.clone());
                while let Some(cls_ptr) = current_class {
                    let methods = cls_ptr.methods.borrow();
                    if let Some(v) = methods.get("init").or_else(|| methods.get("constructor")) {
                        constructor_val = Some(v.clone());
                        break;
                    }
                    current_class = cls_ptr.superclass.clone();
                }

                if let Some(constructor_val) = constructor_val {
                    if constructor_val.is_obj() {
                        let constructor_obj = constructor_val.as_obj();
                        match &*constructor_obj {
                            Obj::Closure(c) => {
                                return self.call_closure(c.clone(), arg_count, callee_reg, dst);
                            }
                            Obj::Function(f) => {
                                let closure = Arc::new(Closure {
                                    function: f.clone(),
                                    upvalues: Vec::new(),
                                });
                                return self.call_closure(closure, arg_count, callee_reg, dst);
                            }
                            _ => {}
                        }
                    }
                }

                // If no constructor, return instance value
                if let Some(dst_reg) = dst {
                    self.stack[caller_offset + dst_reg as usize] = inst_val;
                } else {
                    let callee_idx = caller_offset + callee_reg as usize;
                    self.stack[callee_idx] = inst_val;
                    self.stack.truncate(callee_idx + 1);
                }
                Ok(())
            }
            Obj::NativeFn(native) => {
                let args_start = caller_offset + callee_reg as usize + 1;
                let mut args = Vec::new();
                for i in 0..arg_count {
                    args.push(self.stack[args_start + i as usize].clone());
                }
                let result = native(self, &args)?;
                if let Some(dst_reg) = dst {
                    self.stack[caller_offset + dst_reg as usize] = result;
                } else {
                    let callee_idx = caller_offset + callee_reg as usize;
                    self.stack[callee_idx] = result;
                    self.stack.truncate(callee_idx + 1);
                }
                Ok(())
            }
            _ => Err(format!("Cannot call object instance directly.")),
        }
    }

    pub(crate) fn call_closure(&mut self, closure: Arc<Closure>, arg_count: u8, callee_reg: u8, dst: Option<u8>) -> Result<(), String> {
        if arg_count as usize != closure.function.arity {
            return Err(format!("Expected {} arguments but got {}.", closure.function.arity, arg_count));
        }

        if self.frames.len() == 512 {
            return Err("Stack overflow.".to_string());
        }

        // Increment profiling counter & update observed types for arguments
        let counter = closure.function.chunk.profiling_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if counter >= 1000 && !closure.function.is_hot.load(std::sync::atomic::Ordering::Relaxed) {
            closure.function.is_hot.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        let caller_offset = if let Ok(f) = self.current_frame() { f.stack_offset } else { 0 };
        let new_stack_offset = caller_offset + callee_reg as usize;

        let needed = new_stack_offset + closure.function.chunk.register_count as usize;
        if needed > super::STACK_MAX {
            return Err("Stack overflow.".to_string());
        }
        if self.stack.len() < needed {
            self.stack.resize(needed, Value::null());
        }
        let args_end = new_stack_offset + 1 + arg_count as usize;
        for i in args_end..needed {
            self.stack[i] = Value::null();
        }

        let frame = CallFrame {
            closure,
            ip: 0,
            stack_offset: new_stack_offset,
            register_count: needed - new_stack_offset,
            dst,
        };
        self.frames.push(frame);
        Ok(())
    }
}
