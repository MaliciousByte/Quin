use std::collections::HashMap;
use std::sync::Arc;
use std::cell::RefCell;
use crate::frontend::chunk::{OpCode, ICEntry};
use crate::value::{Value, Closure};
use crate::vm::obj::Obj;
use super::{VM, CallFrame};

impl VM {
    pub(crate) fn execute_op(&mut self, op: OpCode) -> Result<(), String> {
        match op {
            OpCode::Constant(idx) => {
                let constant = self.read_constant(idx)?;
                self.push(constant);
            }
            OpCode::Null => self.push(Value::null()),
            OpCode::True => self.push(Value::bool(true)),
            OpCode::False => self.push(Value::bool(false)),
            OpCode::Pop => { self.pop()?; }
            OpCode::Dup => {
                let val = self.peek(0)?.clone();
                self.push(val);
            }
            
            OpCode::Closure(idx) => {
                let function_val = self.read_constant(idx)?;
                if function_val.is_obj() {
                    if let Obj::Function(function) = &*function_val.as_obj() {
                        let mut upvalues = Vec::new();
                        for req in &function.upvalues {
                            if req.is_local {
                                upvalues.push(self.capture_upvalue(self.current_frame()?.stack_offset + req.index));
                            } else {
                                upvalues.push(self.current_frame()?.closure.upvalues[req.index].clone());
                            }
                        }
                        let closure = Arc::new(Closure {
                            function: function.clone(),
                            upvalues,
                        });
                        self.push(Value::obj(Arc::new(Obj::Closure(closure))));
                    } else {
                        return Err("Expected function for closure.".to_string());
                    }
                } else {
                    return Err("Expected function object for closure.".to_string());
                }
            }

            OpCode::GetUpvalue(idx) => {
                let upvalue = self.current_frame()?.closure.upvalues[idx].clone();
                let val = match &upvalue.borrow().closed {
                    Some(val) => val.clone(),
                    None => self.stack[upvalue.borrow().index].clone(),
                };
                self.push(val);
            }
            OpCode::SetUpvalue(idx) => {
                let val = self.peek(0)?.clone();
                let upvalue = self.current_frame()?.closure.upvalues[idx].clone();
                if upvalue.borrow().closed.is_some() {
                    upvalue.borrow_mut().closed = Some(val);
                } else {
                    let index = upvalue.borrow().index;
                    self.stack[index] = val;
                }
            }
            OpCode::CloseUpvalue => {
                self.close_upvalues(self.stack.len() - 1);
                self.pop()?;
            }
            OpCode::GetLocal(idx) => {
                let offset = self.current_frame()?.stack_offset;
                self.push(self.stack[offset + idx].clone());
            }
            OpCode::SetLocal(idx) => {
                let offset = self.current_frame()?.stack_offset;
                let val = self.peek(0)?.clone();
                self.stack[offset + idx] = val;
            }
            OpCode::GetGlobal(idx) => {
                let name = self.read_string(idx)?;
                if let Some(val) = self.globals.get(&name) {
                    self.push(val.clone());
                } else {
                    return Err(format!("Undefined variable '{}'.", name));
                }
            }
            OpCode::SetGlobal(idx) => {
                let name = self.read_string(idx)?;
                if self.globals.contains_key(&name) {
                    let val = self.peek(0)?.clone();
                    self.globals.insert(name, val);
                } else {
                    return Err(format!("Undefined variable '{}'.", name));
                }
            }
            OpCode::DefineGlobal(idx) => {
                let name = self.read_string(idx)?;
                let val = self.pop()?;
                self.globals.insert(name, val);
            }
            OpCode::ImportModule(name_idx) => {
                let name = self.read_string(name_idx)?;
                self.load_module(name, &[])?;
            }
            OpCode::ImportItems(name_idx, count) => {
                let name = self.read_string(name_idx)?;
                let mut items = Vec::new();
                for _ in 0..count {
                    let item_val = self.pop()?;
                    if let Obj::String(s) = &*item_val.as_obj() {
                        items.push(s.clone());
                    } else {
                        return Err("Import item must be a string.".to_string());
                    }
                }
                items.reverse();
                self.load_module(name, &items)?;
            }

            OpCode::Equal => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(Value::bool(a == b));
            }
            OpCode::Greater => self.binary_op_bool(|a, b| a > b)?,
            OpCode::Less => self.binary_op_bool(|a, b| a < b)?,
            OpCode::Add => {
                let b = self.pop()?;
                let a = self.pop()?;
                if a.is_int() && b.is_int() {
                    self.push(Value::int(a.as_int() + b.as_int()));
                } else if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
                    let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
                    let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
                    self.push(Value::float(va + vb));
                } else if a.is_obj() || b.is_obj() {
                    let res = format!("{}{}", a, b);
                    let interned = self.intern(&res);
                    self.push(Value::obj(Arc::new(Obj::String(interned))));
                } else {
                    return Err("Operands must be two numbers or include a string.".to_string())
                }
            }
            OpCode::Subtract => self.binary_op_math(|a, b| a - b, |a, b| a - b)?,
            OpCode::Multiply => self.binary_op_math(|a, b| a * b, |a, b| a * b)?,
            OpCode::Divide => self.binary_op_math(|a, b| a / b, |a, b| a / b)?,
            
            OpCode::Not => {
                let val = self.pop()?;
                self.push(Value::bool(self.is_falsey(&val)));
            }
            OpCode::Negate => {
                let val = self.pop()?;
                if val.is_int() {
                    self.push(Value::int(-val.as_int()));
                } else if val.is_float() {
                    self.push(Value::float(-val.as_float()));
                } else {
                    return Err("Operand must be a number.".to_string());
                }
            }

            OpCode::JumpIfFalse(offset) => {
                let val = self.peek(0)?;
                if self.is_falsey(val) {
                    self.current_frame_mut()?.ip += offset;
                }
            }
            OpCode::Jump(offset) => {
                self.current_frame_mut()?.ip += offset;
            }
            OpCode::Loop(offset) => {
                let is_hot = {
                    let frame = self.current_frame_mut()?;
                    frame.ip -= offset;
                    frame.closure.function.increment_hotness()
                };

                if is_hot {
                    let closure = self.current_frame()?.closure.clone();
                    let native_ptr = self.jit_engine.compile(&closure.function);

                    if !native_ptr.is_null() && self.jit_recursion_depth < 500 {
                        closure.function.native_ptr.store(
                            native_ptr as *mut u8,
                            std::sync::atomic::Ordering::Relaxed,
                        );

                        // OSR: pop interpreter frame, restart function in JIT from IP=0.
                        let frame = self.frames.pop().unwrap();
                        let stack_offset = frame.stack_offset;

                        let needed = stack_offset + closure.function.max_locals;
                        if needed > super::STACK_MAX {
                            return Err("Stack overflow".into());
                        }
                        if self.stack.len() < needed {
                            self.stack.resize(needed, Value::null());
                        }

                        let native_fn: extern "C" fn(*mut VM, *const Value) -> Value =
                            unsafe { std::mem::transmute(native_ptr) };
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
                            // Do NOT return — let interpreter continue from deopt frame
                        } else {
                            // JIT finished — push result and return to caller immediately.
                            self.stack.truncate(stack_offset);
                            self.push(result);
                            return Ok(());
                        }
                    }
                    // JIT bailed (null ptr) — continue interpreting silently
                }
            }
            OpCode::JumpIfNull(offset) => {
                let val = self.peek(0)?;
                if val.is_null() {
                    self.current_frame_mut()?.ip += offset;
                }
            }

            OpCode::Call(arg_count) => {
                self.call_value(arg_count)?;
            }
            OpCode::Return => {
                let result = self.pop()?;
                let frame = self.frames.pop().unwrap();
                self.close_upvalues(frame.stack_offset);
                self.stack.truncate(frame.stack_offset);
                self.push(result);
            }

            OpCode::BuildArray(count) => {
                let mut elements = Vec::new();
                for _ in 0..count {
                    elements.push(self.pop()?);
                }
                elements.reverse();
                self.push(Value::obj(Arc::new(Obj::Array(RefCell::new(elements)))));
            }

            OpCode::BuildDict(count) => {
                let mut map = HashMap::new();
                for _ in 0..count {
                    let value = self.pop()?;
                    let key = self.pop()?;
                    map.insert(key, value);
                }
                self.push(Value::obj(Arc::new(Obj::Dict(RefCell::new(map)))));
            }

            OpCode::BuildTuple(count) => {
                let mut elements = Vec::new();
                for _ in 0..count {
                    elements.push(self.pop()?);
                }
                elements.reverse();
                self.push(Value::obj(Arc::new(Obj::Tuple(elements))));
            }

            OpCode::BuildSet(count) => {
                let mut set = std::collections::HashSet::new();
                for _ in 0..count {
                    set.insert(self.pop()?);
                }
                self.push(Value::obj(Arc::new(Obj::Set(RefCell::new(set)))));
            }
            
            OpCode::GetIndex => {
                let index = self.pop()?;
                let target = self.pop()?;
                if target.is_obj() {
                    match &*target.as_obj() {
                        Obj::Array(arr) => {
                            if index.is_int() {
                                let elements = arr.borrow();
                                let i = index.as_int();
                                if i >= 0 && (i as usize) < elements.len() {
                                    self.push(elements[i as usize].clone());
                                } else {
                                    return Err(format!("Array index out of bounds: {}", i));
                                }
                            } else {
                                return Err("Array index must be an integer.".to_string());
                            }
                        }
                        Obj::Dict(map) => {
                            if let Some(val) = map.borrow().get(&index) {
                                self.push(val.clone());
                            } else {
                                self.push(Value::null());
                            }
                        }
                        Obj::Tuple(elements) => {
                            if index.is_int() {
                                let i = index.as_int();
                                if i >= 0 && (i as usize) < elements.len() {
                                    self.push(elements[i as usize].clone());
                                } else {
                                    return Err(format!("Tuple index out of bounds: {}", i));
                                }
                            } else {
                                return Err("Tuple index must be an integer.".to_string());
                            }
                        }
                        _ => return Err("Only arrays, dicts, and tuples can be indexed.".to_string()),
                    }
                } else {
                    return Err("Target is not indexable.".to_string());
                }
            }

            OpCode::SetIndex => {
                let value = self.pop()?;
                let index = self.pop()?;
                let target = self.pop()?;
                if target.is_obj() {
                    match &*target.as_obj() {
                        Obj::Array(arr) => {
                            if index.is_int() {
                                let mut elements = arr.borrow_mut();
                                let i = index.as_int();
                                if i >= 0 && (i as usize) < elements.len() {
                                    elements[i as usize] = value.clone();
                                    self.push(value);
                                } else {
                                    return Err(format!("Array index out of bounds: {}", i));
                                }
                            } else {
                                return Err("Array index must be an integer.".to_string());
                            }
                        }
                        Obj::Dict(map) => {
                            map.borrow_mut().insert(index, value.clone());
                            self.push(value);
                        }
                        _ => return Err("Only arrays and dicts can be indexed for assignment.".to_string()),
                    }
                } else {
                    return Err("Target is not indexable for assignment.".to_string());
                }
            }
            
            OpCode::BuildInstance(name_idx, fields_count) => {
                let name = self.read_string(name_idx)?;
                let mut pairs = Vec::with_capacity(fields_count as usize * 2);
                for _ in 0..fields_count {
                    pairs.push(self.pop()?); // value
                    pairs.push(self.pop()?); // name
                }
                pairs.reverse();

                let mut field_values = Vec::with_capacity(fields_count as usize);
                let mut current_shape = self.root_shape.clone();

                for i in (0..pairs.len()).step_by(2) {
                    let field_name_val = &pairs[i];
                    let field_val = &pairs[i+1];
                    
                    let field_name = if field_name_val.is_obj() {
                        if let Obj::String(s) = &*field_name_val.as_obj() {
                            s.clone()
                        } else {
                            return Err("Field name must be a string.".to_string());
                        }
                    } else {
                        return Err("Field name must be a string.".to_string());
                    };
                    
                    // Transition shape
                    let existing = current_shape.transitions.borrow().get(&field_name).cloned();
                    let next_shape = if let Some(next) = existing {
                        next
                    } else {
                        let next = current_shape.transition(field_name.clone(), self.next_shape_id);
                        self.next_shape_id += 1;
                        current_shape.transitions.borrow_mut().insert(field_name, next.clone());
                        next
                    };
                    current_shape = next_shape;
                    field_values.push(field_val.clone());
                }

                let inst = crate::value::Instance {
                    name,
                    shape: current_shape,
                    fields: field_values,
                };
                self.push(Value::obj(Arc::new(Obj::Instance(Arc::new(RefCell::new(inst))))));
            }

            OpCode::GetProperty(name_idx) => {
                let name = self.read_string(name_idx)?;
                let obj = self.peek(0)?.clone();
                
                let ip = self.frames.last().unwrap().ip - 1;
                let mut cached_offset = None;
                
                if obj.is_obj() {
                    match &*obj.as_obj() {
                        Obj::Instance(inst) => {
                            let inst_ptr = inst.borrow();
                            // IC check
                            if let Some(ic) = self.frames.last().unwrap().closure.function.chunk.property_caches.borrow()[ip] {
                                if ic.last_shape_id == inst_ptr.shape.id {
                                    cached_offset = Some(ic.offset);
                                }
                            }
                            
                            let offset = if let Some(off) = cached_offset {
                                off
                            } else if let Some(&off) = inst_ptr.shape.property_offsets.get(&name) {
                                // Update IC
                                let ic = ICEntry { last_shape_id: inst_ptr.shape.id, offset: off };
                                self.frames.last_mut().unwrap().closure.function.chunk.property_caches.borrow_mut()[ip] = Some(ic);
                                off
                            } else {
                                return Err(format!("Property '{}' not found on instance.", name));
                            };
                            
                            self.pop()?; // object
                            self.push(inst_ptr.fields[offset].clone());
                        }
                        Obj::Object(obj_val) => {
                            let obj_ptr = obj_val.borrow();
                            // IC check
                            if let Some(ic) = self.frames.last().unwrap().closure.function.chunk.property_caches.borrow()[ip] {
                                if ic.last_shape_id == obj_ptr.shape.id {
                                    cached_offset = Some(ic.offset);
                                }
                            }
                            
                            if let Some(offset) = cached_offset {
                                self.pop()?;
                                self.push(obj_ptr.fields.borrow()[offset].clone());
                            } else if let Some(&offset) = obj_ptr.shape.property_offsets.get(&name) {
                                // Update IC
                                let ic = ICEntry { last_shape_id: obj_ptr.shape.id, offset };
                                self.frames.last_mut().unwrap().closure.function.chunk.property_caches.borrow_mut()[ip] = Some(ic);
                                self.pop()?;
                                self.push(obj_ptr.fields.borrow()[offset].clone());
                            } else {
                                // Lookup in class hierarchy
                                let mut current_class = Some(obj_ptr.class.clone());
                                let mut found = false;
                                while let Some(cls) = current_class {
                                    if let Some(method_val) = cls.methods.borrow().get(&name) {
                                        if method_val.is_obj() {
                                            match &*method_val.as_obj() {
                                                Obj::Function(method) => {
                                                    self.pop()?;
                                                    self.push(Value::obj(Arc::new(Obj::BoundMethod(Arc::new(crate::value::BoundMethodValue {
                                                        receiver: Value::obj(Arc::new(Obj::Object(obj_val.clone()))),
                                                        method: method.clone(),
                                                    })))));
                                                    found = true;
                                                    break;
                                                }
                                                Obj::Closure(closure) => {
                                                    self.pop()?;
                                                    self.push(Value::obj(Arc::new(Obj::BoundMethod(Arc::new(crate::value::BoundMethodValue {
                                                        receiver: Value::obj(Arc::new(Obj::Object(obj_val.clone()))),
                                                        method: closure.function.clone(),
                                                    })))));
                                                    found = true;
                                                    break;
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    current_class = cls.superclass.clone();
                                }
                                if !found {
                                    return Err(format!("Property '{}' not found on instance of {}.", name, obj_ptr.class.name));
                                }
                            }
                        }
                        Obj::Dict(map) => {
                            let key = Value::obj(Arc::new(Obj::String(name)));
                            self.pop()?; // pop obj
                            if let Some(val) = map.borrow().get(&key) {
                                self.push(val.clone());
                            } else {
                                self.push(Value::null());
                            }
                        }
                        Obj::Class(cls_val) => {
                            // Static method access with hierarchy
                            let mut current_class = Some(cls_val.clone());
                            let mut found = false;
                            while let Some(cls) = current_class {
                                if let Some(method) = cls.methods.borrow().get(&name) {
                                    self.pop()?; // pop obj
                                    self.push(method.clone());
                                    found = true;
                                    break;
                                }
                                current_class = cls.superclass.clone();
                            }
                            if !found {
                                self.pop()?; // pop obj
                                self.push(Value::null());
                            }
                        }
                        _ => return Err("Only instances, objects, classes, and dicts have properties.".to_string()),
                    }
                } else {
                    return Err("Target is not an object.".to_string());
                }
            }
            
            OpCode::SetProperty(name_idx) => {
                let name = self.read_string(name_idx)?;
                let value = self.pop()?;
                let obj = self.pop()?;
                
                let ip = self.frames.last().unwrap().ip - 1;
                
                if obj.is_obj() {
                    match &*obj.as_obj() {
                        Obj::Instance(inst) => {
                            let mut inst_ptr = inst.borrow_mut();
                            
                            let mut cached_offset = None;
                            if let Some(ic) = self.frames.last().unwrap().closure.function.chunk.property_caches.borrow()[ip] {
                                if ic.last_shape_id == inst_ptr.shape.id {
                                    cached_offset = Some(ic.offset);
                                }
                            }
                            
                            if let Some(offset) = cached_offset {
                                inst_ptr.fields[offset] = value.clone();
                            } else if let Some(&offset) = inst_ptr.shape.property_offsets.get(&name) {
                                // Update IC
                                let ic = ICEntry { last_shape_id: inst_ptr.shape.id, offset };
                                self.frames.last_mut().unwrap().closure.function.chunk.property_caches.borrow_mut()[ip] = Some(ic);
                                inst_ptr.fields[offset] = value.clone();
                            } else {
                                // Shape transition
                                let existing_transition = inst_ptr.shape.transitions.borrow().get(&name).cloned();
                                let next_shape = if let Some(s) = existing_transition {
                                    s
                                } else {
                                    let ns = inst_ptr.shape.transition(name.clone(), self.next_shape_id);
                                    self.next_shape_id += 1;
                                    inst_ptr.shape.transitions.borrow_mut().insert(name.clone(), ns.clone());
                                    ns
                                };
                                
                                let offset = next_shape.property_offsets.get(&name).cloned().unwrap();
                                let ic = ICEntry { last_shape_id: next_shape.id, offset };
                                self.frames.last_mut().unwrap().closure.function.chunk.property_caches.borrow_mut()[ip] = Some(ic);
                                
                                inst_ptr.shape = next_shape;
                                inst_ptr.fields.push(value.clone());
                            }
                            self.push(value);
                        }
                        Obj::Object(obj_val) => {
                            let mut obj_ptr = obj_val.borrow_mut();
                            
                            let mut cached_offset = None;
                            if let Some(ic) = self.frames.last().unwrap().closure.function.chunk.property_caches.borrow()[ip] {
                                if ic.last_shape_id == obj_ptr.shape.id {
                                    cached_offset = Some(ic.offset);
                                }
                            }
                            
                            if let Some(offset) = cached_offset {
                                obj_ptr.fields.borrow_mut()[offset] = value.clone();
                            } else if let Some(&offset) = obj_ptr.shape.property_offsets.get(&name) {
                                // Update IC
                                let ic = ICEntry { last_shape_id: obj_ptr.shape.id, offset };
                                self.frames.last_mut().unwrap().closure.function.chunk.property_caches.borrow_mut()[ip] = Some(ic);
                                obj_ptr.fields.borrow_mut()[offset] = value.clone();
                            } else {
                                // Shape transition
                                let existing_transition = obj_ptr.shape.transitions.borrow().get(&name).cloned();
                                let next_shape = if let Some(s) = existing_transition {
                                    s
                                } else {
                                    let ns = obj_ptr.shape.transition(name.clone(), self.next_shape_id);
                                    self.next_shape_id += 1;
                                    obj_ptr.shape.transitions.borrow_mut().insert(name.clone(), ns.clone());
                                    ns
                                };
                                
                                let offset = next_shape.property_offsets.get(&name).cloned().unwrap();
                                let ic = ICEntry { last_shape_id: next_shape.id, offset };
                                self.frames.last_mut().unwrap().closure.function.chunk.property_caches.borrow_mut()[ip] = Some(ic);
                                
                                obj_ptr.shape = next_shape;
                                obj_ptr.fields.borrow_mut().push(value.clone());
                            }
                            self.push(value);
                        }
                        Obj::Dict(map) => {
                            let key = Value::obj(Arc::new(Obj::String(name)));
                            map.borrow_mut().insert(key, value.clone());
                            self.push(value);
                        }
                        Obj::Class(cls_val) => {
                            cls_val.methods.borrow_mut().insert(name, value.clone());
                            self.push(value);
                        }
                        _ => return Err("Only instances, objects, classes, and dicts have properties.".to_string()),
                    }
                } else {
                    return Err("Target is not an object.".to_string());
                }
            }

            OpCode::Throw => {
                let error = self.pop()?;
                self.handle_exception(error)?;
            }

            OpCode::SetupHandler(offset) => {
                let frame_idx = self.frames.len() - 1;
                let stack_idx = self.stack.len();
                let catch_ip = self.current_frame()?.ip + offset;
                self.handlers.push(super::ExceptionHandler {
                    frame_idx,
                    stack_idx,
                    catch_ip,
                });
            }

            OpCode::PopHandler => {
                self.handlers.pop();
            }

            OpCode::BuildClass(name_idx) => {
                let name = self.read_string(name_idx)?;
                let super_val = self.pop()?;
                let mut superclass = None;
                if super_val.is_obj() {
                    if let Obj::Class(cls) = &*super_val.as_obj() {
                        superclass = Some(cls.clone());
                    }
                }
                let cls = Arc::new(crate::value::ClassValue { name, superclass, methods: RefCell::new(HashMap::new()) });
                self.push(Value::obj(Arc::new(Obj::Class(cls))));
            }

            OpCode::Method(name_idx) => {
                let name = self.read_string(name_idx)?;
                let method = self.pop()?;
                let class_val = self.peek(0)?;
                if class_val.is_obj() {
                    if let Obj::Class(cls) = &*class_val.as_obj() {
                        cls.methods.borrow_mut().insert(name, method);
                    } else {
                        return Err("Method opcode applied to non-class object.".to_string());
                    }
                } else {
                    return Err("Method opcode applied to non-class.".to_string());
                }
            }

        }
        Ok(())
    }
}
