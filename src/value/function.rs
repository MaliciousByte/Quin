use std::sync::Arc;
use std::cell::RefCell;
use crate::frontend::chunk::Chunk;
use super::Value;

pub struct Function {
    pub name: Arc<str>,
    pub arity: usize,
    pub max_locals: usize,
    pub is_async: bool,
    pub chunk: Chunk,
    pub upvalues: Vec<UpvalueRequirement>,

    // JIT profiler data
    pub call_count: std::sync::atomic::AtomicU32,
    pub is_hot: std::sync::atomic::AtomicBool,
    pub native_ptr: std::sync::atomic::AtomicPtr<u8>,
}

impl Clone for Function {
    fn clone(&self) -> Self {
        Function {
            name: self.name.clone(),
            arity: self.arity,
            max_locals: self.max_locals,
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
        if count >= 50 && !self.is_hot.load(std::sync::atomic::Ordering::Relaxed) {
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
