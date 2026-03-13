use std::fmt;
use std::sync::Arc;
use std::cell::RefCell;
use std::collections::HashMap;
use crate::chunk::Chunk;
use crate::obj::Obj;

#[repr(transparent)]
pub struct Value(pub u64);

const QNAN: u64 = 0x7ff8000000000000;
const SIGN_BIT: u64 = 0x8000000000000000;
const TAG_NULL: u64 = 0x0001000000000000;
const TAG_FALSE: u64 = 0x0002000000000000;
const TAG_TRUE: u64 = 0x0003000000000000;
const TAG_INT: u64 = 0x0004000000000000;
const TAG_DEOPT: u64 = 0x0007000000000000;

impl Value {
    pub fn null() -> Self { Value(QNAN | TAG_NULL) }
    pub fn bool(b: bool) -> Self {
        if b { Value(QNAN | TAG_TRUE) }
        else { Value(QNAN | TAG_FALSE) }
    }
    pub fn float(f: f64) -> Self {
        Value(f.to_bits())
    }
    pub fn int(i: i64) -> Self {
        Value(QNAN | TAG_INT | (i as u64 & 0x0000FFFFFFFFFFFF))
    }
    pub fn obj(obj: Arc<Obj>) -> Self {
        let ptr = Arc::into_raw(obj) as u64;
        Value(SIGN_BIT | QNAN | ptr)
    }

    pub fn is_float(&self) -> bool { (self.0 & QNAN) != QNAN }
    pub fn is_null(&self) -> bool { self.0 == (QNAN | TAG_NULL) }
    pub fn is_bool(&self) -> bool { self.0 == (QNAN | TAG_FALSE) || self.0 == (QNAN | TAG_TRUE) }
    pub fn is_int(&self) -> bool { (self.0 & 0xFFFF000000000000) == (QNAN | TAG_INT) }
    pub fn is_obj(&self) -> bool { (self.0 & (SIGN_BIT | QNAN)) == (SIGN_BIT | QNAN) }
    pub fn is_deopt(&self) -> bool { (self.0 & 0xFFFF000000000000) == (QNAN | TAG_DEOPT) }

    pub fn deopt(ip: usize) -> Self {
        Value(QNAN | TAG_DEOPT | (ip as u64 & 0x0000FFFFFFFFFFFF))
    }

    pub fn as_deopt(&self) -> usize {
        (self.0 & 0x0000FFFFFFFFFFFF) as usize
    }

    pub fn as_float(&self) -> f64 { f64::from_bits(self.0) }
    pub fn as_bool(&self) -> bool { self.0 == (QNAN | TAG_TRUE) }
    pub fn as_int(&self) -> i64 {
        let bits = self.0 & 0x0000FFFFFFFFFFFF;
        // Sign extend from 48 bits if necessary.
        if bits & 0x0000800000000000 != 0 {
            (bits | 0xFFFF000000000000) as i64
        } else {
            bits as i64
        }
    }
    pub fn as_obj(&self) -> Arc<Obj> {
        let ptr = (self.0 & ! (SIGN_BIT | QNAN)) as *const Obj;
        unsafe { 
            let rc = Arc::from_raw(ptr);
            let cloned = Arc::clone(&rc);
            std::mem::forget(rc); // Don't decrement count here
            cloned
        }
    }

    // Manual reference counting for common operations
    pub fn mark(&self) {
        if self.is_obj() {
            let ptr = (self.0 & ! (SIGN_BIT | QNAN)) as *const Obj;
            unsafe { Arc::increment_strong_count(ptr); }
        }
    }

    pub fn unmark(&self) {
        if self.is_obj() {
            let ptr = (self.0 & ! (SIGN_BIT | QNAN)) as *const Obj;
            unsafe { Arc::decrement_strong_count(ptr); }
        }
    }
}

// We need to override Value's behavior to handle its pseudo-Drop
// but we can't implement Drop for Copy. So we must be careful.
// Instead of #[derive(Copy, Clone)], we'll go manual if we want full safety.
// BUT, for a VM, we usually explicitly manage stack/globals.
// Let's remove Copy and implement Clone and Drop.

impl Clone for Value {
    fn clone(&self) -> Self {
        self.mark();
        Value(self.0)
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        self.unmark();
    }
}

pub struct Function {
    pub name: Arc<str>,
    pub arity: usize,
    pub is_async: bool,
    pub chunk: Chunk,
    pub upvalues: Vec<UpvalueRequirement>,

    // Profiler data
    pub call_count: std::sync::atomic::AtomicU32,
    pub is_hot: std::sync::atomic::AtomicBool,
    pub native_ptr: std::sync::atomic::AtomicPtr<u8>,
}

impl Clone for Function {
    fn clone(&self) -> Self {
        Function {
            name: self.name.clone(),
            arity: self.arity,
            is_async: self.is_async,
            chunk: self.chunk.clone(),
            upvalues: self.upvalues.clone(),
            call_count: std::sync::atomic::AtomicU32::new(self.call_count.load(std::sync::atomic::Ordering::Relaxed)),
            is_hot: std::sync::atomic::AtomicBool::new(self.is_hot.load(std::sync::atomic::Ordering::Relaxed)),
            native_ptr: std::sync::atomic::AtomicPtr::new(self.native_ptr.load(std::sync::atomic::Ordering::Relaxed)),
        }
    }
}

impl Function {
    pub fn increment_hotness(&self) -> bool {
        let count = self.call_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count >= 1000 && !self.is_hot.load(std::sync::atomic::Ordering::Relaxed) {
            self.is_hot.store(true, std::sync::atomic::Ordering::Relaxed);
            return true;
        }
        false
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UpvalueRequirement {
    pub is_local: bool,
    pub index: usize,
}

pub struct Instance {
    pub name: Arc<str>,
    pub shape: Arc<Shape>,
    pub fields: Vec<Value>,
}

#[derive(Clone)]
pub struct Shape {
    pub id: usize,
    pub property_offsets: HashMap<Arc<str>, usize>,
    pub transitions: RefCell<HashMap<Arc<str>, Arc<Shape>>>,
}

impl Shape {
    pub fn new(id: usize) -> Self {
        Shape {
            id,
            property_offsets: HashMap::new(),
            transitions: RefCell::new(HashMap::new()),
        }
    }

    pub fn transition(&self, name: Arc<str>, next_id: usize) -> Arc<Shape> {
        let mut offsets = self.property_offsets.clone();
        offsets.insert(name, offsets.len());
        Arc::new(Shape {
            id: next_id,
            property_offsets: offsets,
            transitions: RefCell::new(HashMap::new()),
        })
    }
}

#[derive(Clone)]
pub struct ClassValue {
    pub name: Arc<str>,
    pub superclass: Option<Arc<ClassValue>>,
    pub methods: RefCell<HashMap<Arc<str>, Value>>,
}

#[derive(Clone)]
pub struct InstanceValue {
    pub class: Arc<ClassValue>,
    pub shape: Arc<Shape>,
    pub fields: RefCell<Vec<Value>>,
}

pub struct Closure {
    pub function: Arc<Function>,
    pub upvalues: Vec<Arc<RefCell<Upvalue>>>,
}

pub struct Upvalue {
    pub index: usize,          // Stack index when open
    pub closed: Option<Value>, // Value when closed
}

#[derive(Clone)]
pub struct BoundMethodValue {
    pub receiver: Value,
    pub method: Arc<Function>,
}

pub type NativeFn = fn(&mut crate::vm::VM, &[Value]) -> Result<Value, String>;

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        if self.0 == other.0 { return true; }
        if self.is_float() && other.is_float() {
            return self.as_float() == other.as_float();
        }
        if self.is_obj() && other.is_obj() {
             match (&*self.as_obj(), &*other.as_obj()) {
                 (Obj::String(a), Obj::String(b)) => return *a == *b,
                 (Obj::Tuple(a), Obj::Tuple(b)) => return a == b,
                 _ => {}
             }
        }
        false
    }
}

impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if self.is_float() {
            self.0.hash(state);
        } else if self.is_obj() {
            match &*self.as_obj() {
                Obj::String(s) => s.hash(state),
                Obj::Tuple(t) => t.hash(state),
                _ => self.0.hash(state),
            }
        } else {
            self.0.hash(state);
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_float() { return write!(f, "{}", self.as_float()); }
        if self.is_null() { return write!(f, "void"); }
        if self.is_bool() { return write!(f, "{}", self.as_bool()); }
        if self.is_int() { return write!(f, "{}", self.as_int()); }
        
        if self.is_obj() {
            match &*self.as_obj() {
                Obj::String(s) => write!(f, "\"{}\"", s),
                Obj::Function(fun) => write!(f, "<fn {}>", fun.name),
                Obj::NativeFn(_) => write!(f, "<native fn>"),
                Obj::Array(arr) => write!(f, "{:?}", arr.borrow()),
                Obj::Dict(d) => write!(f, "dict{:?}", d.borrow()),
                Obj::Tuple(t) => write!(f, "tuple{:?}", t),
                Obj::Set(s) => write!(f, "set{:?}", s.borrow()),
                Obj::Instance(inst) => {
                    let inst = inst.borrow();
                    write!(f, "<struct instance {} {{", inst.name)?;
                    for (name, &offset) in &inst.shape.property_offsets {
                        write!(f, "{}: {:?}, ", name, inst.fields[offset])?;
                    }
                    write!(f, "}}>")
                }
                Obj::Class(cls) => write!(f, "<class {}>", cls.name),
                Obj::Object(obj) => {
                    let obj = obj.borrow();
                    write!(f, "<instance of {} {{", obj.class.name)?;
                    let fields = obj.fields.borrow();
                    for (name, &offset) in &obj.shape.property_offsets {
                        write!(f, "{}: {:?}, ", name, fields[offset])?;
                    }
                    write!(f, "}}>")
                }
                Obj::Closure(cl) => write!(f, "<closure {}>", cl.function.name),
                Obj::BoundMethod(bm) => write!(f, "<bound method {}>", bm.method.name),
            }
        } else {
            write!(f, "<invalid value {:x}>", self.0)
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_obj() {
            if let Obj::String(s) = &*self.as_obj() {
                return write!(f, "{}", s);
            }
        }
        write!(f, "{:?}", self)
    }
}
