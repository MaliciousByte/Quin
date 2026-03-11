use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use crate::chunk::OpCode;
use crate::value::{Value, Function, InstanceValue};
use crate::obj::Obj;
use crate::jit::JitEngine;
use crate::interner::StringInterner;

const STACK_MAX: usize = 256;

// Use Closure from value.rs

struct CallFrame {
    closure: Rc<crate::value::Closure>,
    ip: usize,
    stack_offset: usize,
}

struct ExceptionHandler {
    frame_idx: usize,
    stack_idx: usize,
    catch_ip: usize,
}

pub struct VM {
    frames: Vec<CallFrame>,
    stack: Vec<Value>,
    pub globals: HashMap<Rc<str>, Value>,
    handlers: Vec<ExceptionHandler>,
    open_upvalues: Vec<Rc<RefCell<crate::value::Upvalue>>>,
    root_shape: Rc<crate::value::Shape>,
    next_shape_id: usize,
    pub jit_engine: JitEngine,
    pub interner: StringInterner,
}

impl VM {
    pub fn new() -> Self {
        let mut vm = VM {
            frames: Vec::new(),
            stack: Vec::with_capacity(STACK_MAX),
            globals: HashMap::new(),
            handlers: Vec::new(),
            open_upvalues: Vec::new(),
            root_shape: Rc::new(crate::value::Shape::new(0)),
            next_shape_id: 1,
            jit_engine: JitEngine::new(),
            interner: StringInterner::new(),
        };

        // Register standard library
        crate::stdlib::register_all(&mut vm);
        
        vm
    }

    pub fn intern(&mut self, s: &str) -> Rc<str> {
        self.interner.intern(s)
    }

    pub fn interpret(&mut self, mut function: Function) -> Result<(), String> {
        self.intern_constants(&mut function);
        let closure = Rc::new(crate::value::Closure {
            function: Rc::new(function),
            upvalues: Vec::new(),
        });
        self.stack.push(Value::obj(Rc::new(Obj::Closure(closure.clone()))));
        self.call_closure(closure, 0)?;

        self.run()
    }

    fn intern_constants(&mut self, function: &mut Function) {
        for i in 0..function.chunk.constants.len() {
            let val = &function.chunk.constants[i];
            if val.is_obj() {
                let obj = val.as_obj();
                match &*obj {
                    Obj::String(s) => {
                        let interned = self.interner.intern(s);
                        function.chunk.constants[i] = Value::obj(Rc::new(Obj::String(interned)));
                    }
                    Obj::Function(f) => {
                        // Cast to mut to intern its constants too
                        let f_ptr = Rc::as_ptr(f) as *mut Function;
                        unsafe {
                            self.intern_constants(&mut *f_ptr);
                        }
                    }
                    Obj::Tuple(elements) => {
                        // Tuple elements might be strings
                        let mut new_elements = Vec::new();
                        for elem in elements {
                            if elem.is_obj() {
                                if let Obj::String(s) = &*elem.as_obj() {
                                    new_elements.push(Value::obj(Rc::new(Obj::String(self.intern(s)))));
                                    continue;
                                }
                            }
                            new_elements.push(elem.clone());
                        }
                        function.chunk.constants[i] = Value::obj(Rc::new(Obj::Tuple(new_elements)));
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn run(&mut self) -> Result<(), String> {
        let starting_depth = self.frames.len();
        loop {
            let op = self.read_instruction()?;
            self.execute_op(op)?;
            if self.frames.len() < starting_depth {
                return Ok(());
            }
        }
    }

    fn execute_op(&mut self, op: OpCode) -> Result<(), String> {
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
                        let closure = Rc::new(crate::value::Closure {
                            function: function.clone(),
                            upvalues,
                        });
                        self.push(Value::obj(Rc::new(Obj::Closure(closure))));
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
                    self.push(Value::obj(Rc::new(Obj::String(interned))));
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
                    let function = {
                        let frame = self.current_frame_mut()?;
                        println!("Function {} is now HOT (loop)!", frame.closure.function.name);
                        frame.closure.function.clone()
                    };
                    let native_ptr = self.jit_engine.compile(&function);
                    function.native_ptr.store(native_ptr as *mut u8, std::sync::atomic::Ordering::Relaxed);
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
                self.push(Value::obj(Rc::new(Obj::Array(RefCell::new(elements)))));
            }

            OpCode::BuildDict(count) => {
                let mut map = HashMap::new();
                for _ in 0..count {
                    let value = self.pop()?;
                    let key = self.pop()?;
                    map.insert(key, value);
                }
                self.push(Value::obj(Rc::new(Obj::Dict(RefCell::new(map)))));
            }

            OpCode::BuildTuple(count) => {
                let mut elements = Vec::new();
                for _ in 0..count {
                    elements.push(self.pop()?);
                }
                elements.reverse();
                self.push(Value::obj(Rc::new(Obj::Tuple(elements))));
            }

            OpCode::BuildSet(count) => {
                let mut set = std::collections::HashSet::new();
                for _ in 0..count {
                    set.insert(self.pop()?);
                }
                self.push(Value::obj(Rc::new(Obj::Set(RefCell::new(set)))));
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
                self.push(Value::obj(Rc::new(Obj::Instance(Rc::new(RefCell::new(inst))))));
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
                                let ic = crate::chunk::ICEntry { last_shape_id: inst_ptr.shape.id, offset: off };
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
                                let ic = crate::chunk::ICEntry { last_shape_id: obj_ptr.shape.id, offset };
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
                                                    self.push(Value::obj(Rc::new(Obj::BoundMethod(Rc::new(crate::value::BoundMethodValue {
                                                        receiver: Value::obj(Rc::new(Obj::Object(obj_val.clone()))),
                                                        method: method.clone(),
                                                    })))));
                                                    found = true;
                                                    break;
                                                }
                                                Obj::Closure(closure) => {
                                                    self.pop()?;
                                                    self.push(Value::obj(Rc::new(Obj::BoundMethod(Rc::new(crate::value::BoundMethodValue {
                                                        receiver: Value::obj(Rc::new(Obj::Object(obj_val.clone()))),
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
                            let key = Value::obj(Rc::new(Obj::String(name)));
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
                                let ic = crate::chunk::ICEntry { last_shape_id: inst_ptr.shape.id, offset };
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
                                let ic = crate::chunk::ICEntry { last_shape_id: next_shape.id, offset };
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
                                let ic = crate::chunk::ICEntry { last_shape_id: obj_ptr.shape.id, offset };
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
                                let ic = crate::chunk::ICEntry { last_shape_id: next_shape.id, offset };
                                self.frames.last_mut().unwrap().closure.function.chunk.property_caches.borrow_mut()[ip] = Some(ic);
                                
                                obj_ptr.shape = next_shape;
                                obj_ptr.fields.borrow_mut().push(value.clone());
                            }
                            self.push(value);
                        }
                        Obj::Dict(map) => {
                            let key = Value::obj(Rc::new(Obj::String(name)));
                            map.borrow_mut().insert(key, value.clone());
                            self.push(value);
                        }
                        _ => return Err("Only instances, objects, and dicts have properties.".to_string()),
                    }
                } else {
                    return Err("Target is not an object.".to_string());
                }
            }

            OpCode::Throw => {
                let error = self.pop()?;
                self.handle_exception(error)?;
            }

            OpCode::Await => {
                // Stub: if it's a promise, we'd yield. For now, just return value if it's immediate.
                let val = self.pop()?;
                self.push(val);
            }

            OpCode::Cast(name_idx) => {
                let _target_type = self.read_string(name_idx)?;
                // Dynamic cast: for now just no-op as everything is Value
                // In a strictly typed VM, we'd check and convert.
            }

            OpCode::SetupHandler(offset) => {
                let frame_idx = self.frames.len() - 1;
                let stack_idx = self.stack.len();
                let catch_ip = self.current_frame()?.ip + offset;
                self.handlers.push(ExceptionHandler {
                    frame_idx,
                    stack_idx,
                    catch_ip,
                });
            }

            OpCode::PopHandler => {
                self.handlers.pop();
            }

            OpCode::Finally => {
                // Logic to pop a handler if we entered it normally?
                // Tricky without a full Try/Finally opcode pairing.
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
                let cls = Rc::new(crate::value::ClassValue { name, superclass, methods: RefCell::new(HashMap::new()) });
                self.push(Value::obj(Rc::new(Obj::Class(cls))));
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

    fn read_instruction(&mut self) -> Result<OpCode, String> {
        let frame = self.current_frame_mut()?;
        if frame.ip >= frame.closure.function.chunk.code.len() {
            return Err("Execution reached end of chunk without returning.".to_string());
        }
        let op = frame.closure.function.chunk.code[frame.ip];
        frame.ip += 1;
        Ok(op)
    }

    fn read_constant(&self, idx: usize) -> Result<Value, String> {
        let frame = self.current_frame()?;
        if idx >= frame.closure.function.chunk.constants.len() {
            return Err(format!("Constant index {} out of bounds.", idx));
        }
        Ok(frame.closure.function.chunk.constants[idx].clone())
    }

    fn read_string(&self, idx: usize) -> Result<Rc<str>, String> {
        let val = self.read_constant(idx)?;
        if val.is_obj() {
            if let Obj::String(s) = &*val.as_obj() {
                return Ok(s.clone());
            }
        }
        Err("Expected string constant.".to_string())
    }

    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Result<Value, String> {
        self.stack.pop().ok_or_else(|| "Stack underflow.".to_string())
    }

    fn peek(&self, distance: usize) -> Result<&Value, String> {
        if distance >= self.stack.len() {
            Err("Stack underflow on peek.".to_string())
        } else {
            Ok(&self.stack[self.stack.len() - 1 - distance])
        }
    }

    pub fn call_value(&mut self, arg_count: u8) -> Result<(), String> {
        let callee = self.peek(arg_count as usize)?.clone();
        if !callee.is_obj() {
            return Err(format!("Can only call functions, closures, and classes. Got: {}", callee));
        }

        match &*callee.as_obj() {
            Obj::Function(fun) => {
                let closure = Rc::new(crate::value::Closure {
                    function: fun.clone(),
                    upvalues: Vec::new(),
                });
                self.call_closure(closure, arg_count)
            }
            Obj::BoundMethod(bm) => {
                let idx = self.stack.len() - arg_count as usize - 1;
                self.stack[idx] = bm.receiver.clone();
                let closure = Rc::new(crate::value::Closure {
                    function: bm.method.clone(),
                    upvalues: Vec::new(),
                });
                self.call_closure(closure, arg_count)
            }
            Obj::Closure(closure) => self.call_closure(closure.clone(), arg_count),
            Obj::Class(cls) => {
                let inst = Rc::new(RefCell::new(InstanceValue {
                    class: cls.clone(),
                    shape: self.root_shape.clone(),
                    fields: RefCell::new(Vec::new()),
                }));
                let idx = self.stack.len() - arg_count as usize - 1;
                self.stack[idx] = Value::obj(Rc::new(Obj::Object(inst.clone())));
                
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
                                let closure = Rc::new(crate::value::Closure {
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

    fn call_closure(&mut self, closure: Rc<crate::value::Closure>, arg_count: u8) -> Result<(), String> {
        if arg_count as usize != closure.function.arity {
            return Err(format!("Expected {} arguments but got {}.", closure.function.arity, arg_count));
        }

        if self.frames.len() == 64 {
            return Err("Stack overflow.".to_string());
        }

        if closure.function.increment_hotness() {
            println!("Function {} is now HOT (call)!", closure.function.name);
            let native_ptr = self.jit_engine.compile(&closure.function);
            closure.function.native_ptr.store(native_ptr as *mut u8, std::sync::atomic::Ordering::Relaxed);
        }

        let native_ptr = closure.function.native_ptr.load(std::sync::atomic::Ordering::Relaxed);
        if !native_ptr.is_null() {
             let native_fn: extern "C" fn(*mut VM, *const Value) -> Value = unsafe { std::mem::transmute(native_ptr) };
             let args_ptr = unsafe { self.stack.as_ptr().add(self.stack.len() - arg_count as usize) };
             let result = native_fn(self as *mut VM, args_ptr);
             
             if !result.is_null() {
                 if result.is_deopt() {
                     let deopt_ip = result.as_deopt();
                     let frame = CallFrame {
                        closure,
                        ip: deopt_ip,
                        stack_offset: self.stack.len() - arg_count as usize - 1,
                     };
                     self.frames.push(frame);
                     return Ok(());
                 }

                 for _ in 0..arg_count + 1 {
                     self.stack.pop();
                 }
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

    fn capture_upvalue(&mut self, index: usize) -> Rc<RefCell<crate::value::Upvalue>> {
        for upvalue in &self.open_upvalues {
            if upvalue.borrow().index == index {
                return upvalue.clone();
            }
        }

        let upvalue = Rc::new(RefCell::new(crate::value::Upvalue {
            index,
            closed: None,
        }));
        self.open_upvalues.push(upvalue.clone());
        upvalue
    }

    fn close_upvalues(&mut self, last_idx: usize) {
        let mut i = 0;
        while i < self.open_upvalues.len() {
            let upvalue_rc = self.open_upvalues[i].clone();
            if upvalue_rc.borrow().index >= last_idx {
                let val = self.stack[upvalue_rc.borrow().index].clone();
                upvalue_rc.borrow_mut().closed = Some(val);
                self.open_upvalues.remove(i);
            } else {
                i += 1;
            }
        }
    }

    pub fn is_falsey(&self, value: &Value) -> bool {
        if value.is_null() { return true; }
        if value.is_bool() { return !value.as_bool(); }
        if value.is_int() { return value.as_int() == 0; }
        if value.is_obj() {
            match &*value.as_obj() {
                Obj::Array(a) => a.borrow().is_empty(),
                Obj::Dict(d) => d.borrow().is_empty(),
                Obj::Set(s) => s.borrow().is_empty(),
                Obj::Object(inst) => inst.borrow().fields.borrow().is_empty(),
                _ => false,
            }
        } else {
            false
        }
    }

    fn binary_op_math<I, F>(&mut self, op_i: I, op_f: F) -> Result<(), String> 
        where I: Fn(i64, i64) -> i64, F: Fn(f64, f64) -> f64 {
        let b = self.pop()?;
        let a = self.pop()?;
        if a.is_int() && b.is_int() {
            self.push(Value::int(op_i(a.as_int(), b.as_int())));
        } else if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
            let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
            let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
            self.push(Value::float(op_f(va, vb)));
        } else {
            return Err("Operands must be numbers.".to_string());
        }
        Ok(())
    }

    fn binary_op_bool<I>(&mut self, op: I) -> Result<(), String> 
        where I: Fn(f64, f64) -> bool {
        let b = self.pop()?;
        let a = self.pop()?;
        if (a.is_int() || a.is_float()) && (b.is_int() || b.is_float()) {
            let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
            let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
            self.push(Value::bool(op(va, vb)));
        } else {
            return Err("Operands must be numbers for comparison.".to_string());
        }
        Ok(())
    }

    fn current_frame(&self) -> Result<&CallFrame, String> {
        self.frames.last().ok_or_else(|| "No call frame.".to_string())
    }

    fn current_frame_mut(&mut self) -> Result<&mut CallFrame, String> {
        self.frames.last_mut().ok_or_else(|| "No call frame.".to_string())
    }

    fn handle_exception(&mut self, error: Value) -> Result<(), String> {
        if let Some(handler) = self.handlers.pop() {
            // Unwind frames
            self.frames.truncate(handler.frame_idx + 1);
            // Unwind stack
            self.stack.truncate(handler.stack_idx);
            // Push error for catch param
            self.push(error);
            // Jump to catch
            self.current_frame_mut()?.ip = handler.catch_ip;
            Ok(())
        } else {
            Err(format!("Uncaught error: {}", error))
        }
    }
}
