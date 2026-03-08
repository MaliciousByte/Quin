use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use crate::chunk::Chunk;

#[derive(Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(Rc<String>),
    Null,
    Function(Rc<Function>), // Original Function variant
    NativeFn(NativeFn),
    Array(Rc<RefCell<Vec<Value>>>),
    Dict(Rc<RefCell<HashMap<Value, Value>>>),
    Tuple(Rc<Vec<Value>>),
    Set(Rc<RefCell<std::collections::HashSet<Value>>>),
    // Original Instance variant (for Struct/Class)
    Instance(Rc<RefCell<Instance>>), 
    Class(Rc<ClassValue>),
    Object(Rc<RefCell<InstanceValue>>),
    Closure(Rc<Closure>),
    BoundMethod(Rc<BoundMethodValue>),
}

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub arity: usize,
    pub is_async: bool,
    pub chunk: Chunk,
}

pub struct Instance {
    pub name: String,
    pub fields: HashMap<String, Value>,
}

#[derive(Clone)]
pub struct ClassValue {
    pub name: String,
    pub superclass: Option<Rc<ClassValue>>,
    pub methods: RefCell<HashMap<String, Value>>,
}

#[derive(Clone)]
pub struct InstanceValue {
    pub class: Rc<ClassValue>,
    pub fields: RefCell<HashMap<String, Value>>,
}

pub struct Closure {
    pub function: Rc<Function>,
}

#[derive(Clone)]
pub struct BoundMethodValue {
    pub receiver: Value,
    pub method: Rc<Function>,
}

pub type NativeFn = fn(&[Value]) -> Result<Value, String>;

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            _ => false, 
        }
    }
}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::Int(i) => i.hash(state),
            Value::String(s) => s.hash(state),
            Value::Tuple(t) => t.hash(state),
            Value::Bool(b) => b.hash(state),
            Value::Null => 0.hash(state),
            _ => 1.hash(state), // Simplified, maps and sets should ideally take hashable values
        }
    }
}

impl Eq for Value {}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Null => write!(f, "void"),
            Value::Function(fun) => write!(f, "<fn {}>", fun.name),
            Value::NativeFn(_) => write!(f, "<native fn>"),
            Value::Array(arr) => write!(f, "{:?}", arr.borrow()),
            Value::Dict(d) => write!(f, "dict{:?}", d.borrow()),
            Value::Tuple(t) => write!(f, "tuple{:?}", t),
            Value::Set(s) => write!(f, "set{:?}", s.borrow()),
            Value::Instance(inst) => write!(f, "<struct instance {}>", inst.borrow().name),
            Value::Class(cls) => write!(f, "<class {}>", cls.name),
            Value::Object(obj) => write!(f, "<instance of {}>", obj.borrow().class.name),
            Value::Closure(cl) => write!(f, "<closure {}>", cl.function.name),
            Value::BoundMethod(bm) => write!(f, "<bound method {}>", bm.method.name),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
