use std::sync::Arc;
use std::cell::RefCell;
use crate::value::{Value, Closure, InstanceValue};
use crate::vm::obj::Obj;
use super::{VM, CallFrame};

impl VM {
    pub fn call_value(&mut self, arg_count: u8) -> Result<(), String> {
        let callee = self.peek(arg_count as usize)?.clone();
        if !callee.is_obj() {
            return Err(format!("Can only call functions, closures, and classes. Got: {}", callee));
        }

        match &*callee.as_obj() {
            Obj::Function(fun) => {
                let closure = Arc::new(Closure {
                    function: fun.clone(),
                    upvalues: Vec::new(),
                });
                self.call_closure(closure, arg_count)
            }
            Obj::BoundMethod(bm) => {
                let idx = self.stack.len() - arg_count as usize - 1;
                self.stack[idx] = bm.receiver.clone();
                let closure = Arc::new(Closure {
                    function: bm.method.clone(),
                    upvalues: Vec::new(),
                });
                self.call_closure(closure, arg_count)
            }
            Obj::Closure(closure) => self.call_closure(closure.clone(), arg_count),
            Obj::Class(cls) => {
                let inst = Arc::new(RefCell::new(InstanceValue {
                    class: cls.clone(),
                    shape: self.root_shape.clone(),
                    fields: RefCell::new(Vec::new()),
                }));
                let idx = self.stack.len() - arg_count as usize - 1;
                self.stack[idx] = Value::obj(Arc::new(Obj::Object(inst.clone())));
                
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
                                let c = c.clone();
                                return self.call_closure(c, arg_count);
                            }
                            Obj::Function(f) => {
                                let closure = Arc::new(Closure {
                                    function: f.clone(),
                                    upvalues: Vec::new(),
                                });
                                return self.call_closure(closure, arg_count);
                            }
                            _ => {}
                        }
                    }
                }
                Ok(())
            }
            Obj::NativeFn(native) => {
                let mut args = Vec::new();
                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();
                let result = native(self, &args)?;
                self.pop()?; // Pop the native fn
                self.push(result);
                Ok(())
            }
            _ => Err(format!("Cannot call object instance directly.")),
        }
    }

    pub(crate) fn call_closure(&mut self, closure: Arc<Closure>, arg_count: u8) -> Result<(), String> {
        if arg_count as usize != closure.function.arity {
            return Err(format!("Expected {} arguments but got {}.", closure.function.arity, arg_count));
        }

        if self.frames.len() == 512 {
            return Err("Stack overflow.".to_string());
        }

        if closure.function.increment_hotness() {
            let native_ptr = self.jit_engine.compile(&closure.function);
            closure.function.native_ptr.store(native_ptr as *mut u8, std::sync::atomic::Ordering::Relaxed);
        }

        let native_ptr = closure.function.native_ptr.load(std::sync::atomic::Ordering::Relaxed);
        if !native_ptr.is_null() && self.jit_recursion_depth < 500 {
             let native_fn: extern "C" fn(*mut VM, *const Value) -> Value = unsafe { std::mem::transmute(native_ptr) };
             let stack_offset = self.stack.len() - arg_count as usize - 1;
             
             // Ensure stack has room for all locals before calling JIT
             if self.stack.len() < stack_offset + closure.function.max_locals {
                 self.stack.resize(stack_offset + closure.function.max_locals, Value::null());
             }
             
             let args_ptr = unsafe { self.stack.as_ptr().add(stack_offset) };
             self.jit_recursion_depth += 1;
             let result = native_fn(self as *mut VM, args_ptr);
             self.jit_recursion_depth -= 1;
             
             if result.is_deopt() {
                 // Type guard failed — resume interpreter at deopt IP.
                 let deopt_ip = result.as_deopt();
                 let frame = CallFrame {
                    closure,
                    ip: deopt_ip,
                    stack_offset,
                 };
                 self.frames.push(frame);
                 return Ok(());
             } else {
                 // JIT returned successfully.
                 self.stack.truncate(stack_offset);
                 self.stack.push(result);
                 return Ok(());
             }
        }

        let frame = CallFrame {
            closure,
            ip: 0,
            stack_offset: self.stack.len() - arg_count as usize - 1,
        };
        self.frames.push(frame);
        Ok(())
    }
}
