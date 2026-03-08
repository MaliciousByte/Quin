use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use crate::chunk::OpCode;
use crate::value::{Value, Function, Instance, ClassValue, InstanceValue, BoundMethodValue};

const STACK_MAX: usize = 256;

pub struct Closure {
    pub function: Rc<Function>,
}

struct CallFrame {
    function: Rc<Function>,
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
    globals: HashMap<String, Value>,
    handlers: Vec<ExceptionHandler>,
}

impl VM {
    pub fn new() -> Self {
        let mut vm = VM {
            frames: Vec::new(),
            stack: Vec::with_capacity(STACK_MAX),
            globals: HashMap::new(),
            handlers: Vec::new(),
        };

        // Built-in emit function
        vm.globals.insert(
            "emit".to_string(), 
            Value::NativeFn(|args| {
                if let Some(val) = args.first() {
                    println!("{}", val);
                } else {
                    println!("");
                }
                Ok(Value::Null)
            })
        );

        vm.globals.insert(
            "sqrt".to_string(),
            Value::NativeFn(|args| {
                if let Some(Value::Float(f)) = args.first() {
                    Ok(Value::Float(f.sqrt()))
                } else {
                    Err("sqrt expects a float".to_string())
                }
            })
        );

        vm.globals.insert(
            "pow".to_string(),
            Value::NativeFn(|args| {
                if args.len() == 2 {
                    if let (Value::Float(base), Value::Float(exp)) = (&args[0], &args[1]) {
                        Ok(Value::Float(base.powf(*exp)))
                    } else {
                        Err("pow expects floats".to_string())
                    }
                } else {
                    Err("pow expects 2 arguments".to_string())
                }
            })
        );
        
        vm
    }

    pub fn interpret(&mut self, function: Function) -> Result<(), String> {
        let frame = CallFrame {
            function: Rc::new(function),
            ip: 0,
            stack_offset: 0,
        };
        self.frames.push(frame);

        self.run()
    }

    fn run(&mut self) -> Result<(), String> {
        loop {
            let op = self.read_instruction()?;

            match op {
                OpCode::Constant(idx) => {
                    let constant = self.read_constant(idx)?;
                    self.push(constant);
                }
                OpCode::Null => self.push(Value::Null),
                OpCode::True => self.push(Value::Bool(true)),
                OpCode::False => self.push(Value::Bool(false)),
                OpCode::Pop => { self.pop()?; }
                OpCode::Dup => {
                    let val = self.peek(0)?.clone();
                    self.push(val);
                }
                
                OpCode::GetLocal(slot) => {
                    let offset = self.current_frame()?.stack_offset;
                    let val = self.stack[offset + slot].clone();
                    self.push(val);
                }
                OpCode::SetLocal(slot) => {
                    let offset = self.current_frame()?.stack_offset;
                    let val = self.peek(0)?.clone();
                    self.stack[offset + slot] = val;
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
                    self.push(Value::Bool(a == b));
                }
                OpCode::Greater => self.binary_op_bool(|a, b| a > b)?,
                OpCode::Less => self.binary_op_bool(|a, b| a < b)?,
                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Int(x + y)),
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Float(x + y)),
                        (Value::Null, Value::String(y)) => {
                            let res = format!("void{}", y);
                            self.push(Value::String(Rc::new(res)));
                        }
                        (Value::String(x), Value::Null) => {
                            let res = format!("{}void", x);
                            self.push(Value::String(Rc::new(res)));
                        }
                        (Value::String(x), y) => {
                            let res = format!("{}{}", x, y);
                            self.push(Value::String(Rc::new(res)));
                        }
                        (x, Value::String(y)) => {
                            let res = format!("{}{}", x, y);
                            self.push(Value::String(Rc::new(res)));
                        }
                        _ => return Err("Operands must be two numbers or include a string.".to_string())
                    }
                }
                OpCode::Subtract => self.binary_op_math(|a, b| a - b, |a, b| a - b)?,
                OpCode::Multiply => self.binary_op_math(|a, b| a * b, |a, b| a * b)?,
                OpCode::Divide => self.binary_op_math(|a, b| a / b, |a, b| a / b)?,
                
                OpCode::Not => {
                    let val = self.pop()?;
                    self.push(Value::Bool(self.is_falsey(&val)));
                }
                OpCode::Negate => {
                    match self.pop()? {
                        Value::Int(n) => self.push(Value::Int(-n)),
                        Value::Float(n) => self.push(Value::Float(-n)),
                        _ => return Err("Operand must be a number.".to_string())
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
                    self.current_frame_mut()?.ip -= offset;
                }
                OpCode::JumpIfNull(offset) => {
                    let val = self.peek(0)?;
                    if matches!(val, Value::Null) {
                        self.current_frame_mut()?.ip += offset;
                    }
                }

                OpCode::Call(arg_count) => {
                    self.call_value(arg_count)?;
                }
                OpCode::Return => {
                    let result = self.pop()?;
                    let frame = self.frames.pop().unwrap();
                    self.stack.truncate(frame.stack_offset);
                    self.push(result);
                    if self.frames.is_empty() {
                        return Ok(());
                    }
                }

                OpCode::BuildArray(count) => {
                    let mut elements = Vec::new();
                    for _ in 0..count {
                        elements.push(self.pop()?);
                    }
                    elements.reverse();
                    self.push(Value::Array(Rc::new(RefCell::new(elements))));
                }

                OpCode::BuildDict(count) => {
                    let mut map = HashMap::new();
                    for _ in 0..count {
                        let value = self.pop()?;
                        let key = self.pop()?;
                        map.insert(key, value);
                    }
                    self.push(Value::Dict(Rc::new(RefCell::new(map))));
                }

                OpCode::BuildTuple(count) => {
                    let mut elements = Vec::new();
                    for _ in 0..count {
                        elements.push(self.pop()?);
                    }
                    elements.reverse();
                    self.push(Value::Tuple(Rc::new(elements)));
                }

                OpCode::BuildSet(count) => {
                    let mut set = std::collections::HashSet::new();
                    for _ in 0..count {
                        set.insert(self.pop()?);
                    }
                    self.push(Value::Set(Rc::new(RefCell::new(set))));
                }
                
                OpCode::GetIndex => {
                    let index = self.pop()?;
                    let target = self.pop()?;
                    match target {
                        Value::Array(arr) => {
                            if let Value::Int(i) = index {
                                let elements = arr.borrow();
                                if i >= 0 && (i as usize) < elements.len() {
                                    self.push(elements[i as usize].clone());
                                } else {
                                    return Err(format!("Array index out of bounds: {}", i));
                                }
                            } else {
                                return Err("Array index must be an integer.".to_string());
                            }
                        }
                        Value::Dict(map) => {
                            if let Some(val) = map.borrow().get(&index) {
                                self.push(val.clone());
                            } else {
                                self.push(Value::Null); // JS style: non-existent key returns null/undefined
                            }
                        }
                        Value::Tuple(elements) => {
                            if let Value::Int(i) = index {
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
                }

                OpCode::SetIndex => {
                    let value = self.pop()?;
                    let index = self.pop()?;
                    let target = self.pop()?;
                    match target {
                        Value::Array(arr) => {
                            if let Value::Int(i) = index {
                                let mut elements = arr.borrow_mut();
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
                        Value::Dict(map) => {
                            map.borrow_mut().insert(index, value.clone());
                            self.push(value);
                        }
                        _ => return Err("Only arrays and dicts can be indexed for assignment.".to_string()),
                    }
                }
                
                OpCode::BuildInstance(name_idx, fields_count) => {
                    let name = self.read_string(name_idx)?;
                    let mut inst = Instance {
                        name,
                        fields: HashMap::new(),
                    };
                    let mut fields = Vec::new();
                    for _ in 0..fields_count {
                        fields.push(self.pop()?);
                    }
                    for _ in 0..fields_count {
                        let val = fields.pop().unwrap();
                        let key = self.pop()?;
                        if let Value::String(s) = key {
                            inst.fields.insert(s.to_string(), val);
                        }
                    }
                    self.push(Value::Instance(Rc::new(RefCell::new(inst))));
                }

                OpCode::GetProperty(name_idx) => {
                    let name = self.read_string(name_idx)?;
                    let obj = self.peek(0)?.clone();
                    let mut has_prop = false;
                    let mut prop = None;
                    match obj {
                        Value::Instance(inst) => {
                            if let Some(val) = inst.borrow().fields.get(&name) {
                                has_prop = true;
                                prop = Some(val.clone());
                            }
                        }
                        Value::Object(obj_val) => {
                            if let Some(val) = obj_val.borrow().fields.borrow().get(&name) {
                                has_prop = true;
                                prop = Some(val.clone());
                            } else {
                                // Lookup in class hierarchy
                                let mut current_class = Some(obj_val.borrow().class.clone());
                                while let Some(cls) = current_class {
                                    if let Some(method_val) = cls.methods.borrow().get(&name) {
                                        if let Value::Function(method) = method_val {
                                            has_prop = true;
                                            prop = Some(Value::BoundMethod(Rc::new(BoundMethodValue {
                                                receiver: Value::Object(obj_val.clone()),
                                                method: method.clone(),
                                            })));
                                            break;
                                        } else if let Value::Closure(closure) = method_val {
                                            has_prop = true;
                                            prop = Some(Value::BoundMethod(Rc::new(BoundMethodValue {
                                                receiver: Value::Object(obj_val.clone()),
                                                method: closure.function.clone(),
                                            })));
                                            break;
                                        }
                                    }
                                    current_class = cls.superclass.clone();
                                }
                            }
                        }
                        Value::Dict(map) => {
                            let key = Value::String(Rc::new(name));
                            if let Some(val) = map.borrow().get(&key) {
                                has_prop = true;
                                prop = Some(val.clone());
                            }
                        }
                        Value::Class(cls_val) => {
                            // Static method access with hierarchy
                            let mut current_class = Some(cls_val);
                            while let Some(cls) = current_class {
                                if let Some(method) = cls.methods.borrow().get(&name) {
                                    has_prop = true;
                                    prop = Some(method.clone());
                                    break;
                                }
                                current_class = cls.superclass.clone();
                            }
                        }
                        _ => return Err("Only instances, objects, and dicts have properties.".to_string()),
                    }
                    self.pop()?; // pop obj
                    if has_prop {
                        self.push(prop.unwrap());
                    } else {
                        self.push(Value::Null);
                    }
                }
                
                OpCode::SetProperty(name_idx) => {
                    let name = self.read_string(name_idx)?;
                    let value = self.pop()?;
                    let obj = self.pop()?;
                    match obj {
                        Value::Instance(inst) => {
                            inst.borrow_mut().fields.insert(name, value.clone());
                            self.push(value);
                        }
                        Value::Object(obj_val) => {
                            obj_val.borrow().fields.borrow_mut().insert(name, value.clone());
                            self.push(value);
                        }
                        Value::Dict(map) => {
                            let key = Value::String(Rc::new(name));
                            map.borrow_mut().insert(key, value.clone());
                            self.push(value);
                        }
                        _ => return Err("Only instances, objects, and dicts have properties.".to_string()),
                    }
                }

                OpCode::Throw => {
                    let error = self.pop()?;
                    return self.handle_exception(error);
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

                OpCode::Finally => {
                    // Logic to pop a handler if we entered it normally?
                    // Tricky without a full Try/Finally opcode pairing.
                }

                OpCode::BuildClass(name_idx) => {
                    let name = self.read_string(name_idx)?;
                    let super_val = self.pop()?;
                    let superclass = if let Value::Class(cls) = super_val {
                        Some(cls)
                    } else {
                        None
                    };
                    let cls = Rc::new(ClassValue { name, superclass, methods: RefCell::new(HashMap::new()) });
                    self.push(Value::Class(cls));
                }

                OpCode::Method(name_idx) => {
                    let name = self.read_string(name_idx)?;
                    let method = self.pop()?;
                    let class_val = self.peek(0)?;
                    if let Value::Class(cls) = class_val {
                        cls.methods.borrow_mut().insert(name, method);
                    } else {
                        return Err("Method opcode applied to non-class.".to_string());
                    }
                }

            }
        }
    }

    fn read_instruction(&mut self) -> Result<OpCode, String> {
        let frame = self.current_frame_mut()?;
        if frame.ip >= frame.function.chunk.code.len() {
            return Err("Execution reached end of chunk without returning.".to_string());
        }
        let op = frame.function.chunk.code[frame.ip];
        frame.ip += 1;
        Ok(op)
    }

    fn read_constant(&self, idx: usize) -> Result<Value, String> {
        let frame = self.current_frame()?;
        if idx >= frame.function.chunk.constants.len() {
            return Err("Instruction referenced missing constant.".to_string());
        }
        Ok(frame.function.chunk.constants[idx].clone())
    }

    fn read_string(&self, idx: usize) -> Result<String, String> {
        match self.read_constant(idx)? {
            Value::String(s) => Ok(s.to_string()),
            _ => Err("Expected string constant.".to_string())
        }
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Result<Value, String> {
        self.stack.pop().ok_or_else(|| "Stack underflow.".to_string())
    }

    fn peek(&self, distance: usize) -> Result<&Value, String> {
        if distance >= self.stack.len() {
            Err("Stack underflow on peek.".to_string())
        } else {
            Ok(&self.stack[self.stack.len() - 1 - distance])
        }
    }

    fn call_value(&mut self, arg_count: u8) -> Result<(), String> {
        let callee = self.peek(arg_count as usize)?.clone();
        match callee {
            Value::Function(fun) => self.call(fun, arg_count),
            Value::BoundMethod(bm) => {
                let idx = self.stack.len() - arg_count as usize - 1;
                self.stack[idx] = bm.receiver.clone();
                self.call(bm.method.clone(), arg_count)
            }
            Value::Closure(closure) => self.call(closure.function.clone(), arg_count),
            Value::Class(cls) => {
                let inst = Rc::new(RefCell::new(InstanceValue {
                    class: cls.clone(),
                    fields: RefCell::new(HashMap::new()),
                }));
                let idx = self.stack.len() - arg_count as usize - 1;
                self.stack[idx] = Value::Object(inst.clone());
                
                if let Some(constructor_val) = cls.methods.borrow().get("constructor") {
                    if let Value::Function(f) = constructor_val {
                        return self.call(f.clone(), arg_count);
                    } else if let Value::Closure(c) = constructor_val {
                        return self.call(c.function.clone(), arg_count);
                    }
                } else if arg_count != 0 {
                    return Err(format!("Expected 0 arguments but got {}.", arg_count));
                }
                Ok(())
            }
            Value::NativeFn(native) => {
                let mut args = Vec::new();
                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();
                let result = native(&args)?;
                self.pop()?; // Pop the native fn
                self.push(result);
                Ok(())
            }
            Value::Object(_inst) => {
                Err("Cannot call object instance directly.".to_string())
            }
            _ => Err(format!("Can only call functions, closures, and classes. Got: {:?}", callee))
        }
    }

    fn call(&mut self, function: Rc<Function>, arg_count: u8) -> Result<(), String> {
        if arg_count as usize != function.arity {
            return Err(format!("Expected {} arguments but got {}.", function.arity, arg_count));
        }

        if self.frames.len() == 64 {
            return Err("Stack overflow.".to_string());
        }

        let frame = CallFrame {
            function,
            ip: 0,
            stack_offset: self.stack.len() - arg_count as usize - 1,
        };
        self.frames.push(frame);
        Ok(())
    }

    fn is_falsey(&self, value: &Value) -> bool {
        match value {
            Value::Null => true,
            Value::Bool(b) => !b,
            Value::Int(0) => true,
            Value::Array(a) => a.borrow().is_empty(),
            Value::Dict(d) => d.borrow().is_empty(),
            Value::Set(s) => s.borrow().is_empty(),
            Value::Object(inst) => inst.borrow().fields.borrow().is_empty(),
            _ => false,
        }
    }

    fn binary_op_math<I, F>(&mut self, op_i: I, op_f: F) -> Result<(), String> 
        where I: Fn(i64, i64) -> i64, F: Fn(f64, f64) -> f64 {
        let b = self.pop()?;
        let a = self.pop()?;
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => self.push(Value::Int(op_i(x, y))),
            (Value::Float(x), Value::Float(y)) => self.push(Value::Float(op_f(x, y))),
            _ => return Err("Operands must be numbers.".to_string())
        }
        Ok(())
    }

    fn binary_op_bool<I>(&mut self, op: I) -> Result<(), String> 
        where I: Fn(f64, f64) -> bool {
        let b = self.pop()?;
        let a = self.pop()?;
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => self.push(Value::Bool(op(x as f64, y as f64))),
            (Value::Float(x), Value::Float(y)) => self.push(Value::Bool(op(x, y))),
            _ => return Err("Operands must be numbers for comparison.".to_string())
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
