pub mod function;
pub mod types;

pub use function::*;
pub use types::*;

use std::fmt;
use std::sync::Arc;
use crate::vm::obj::Obj;

// ─────────────────────────────────────────────────────────────────────────────
// NaN-boxed Value representation
//
// Layout (64-bit):
//   float:  any bit pattern where (bits & QNAN) != QNAN
//   null:   QNAN | TAG_NULL
//   false:  QNAN | TAG_FALSE
//   true:   QNAN | TAG_TRUE
//   int:    QNAN | TAG_INT | (48-bit payload)
//   deopt:  QNAN | TAG_DEOPT | (48-bit IP)
//   obj:    SIGN_BIT | QNAN | (pointer)
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) const QNAN: u64 = 0x7ff8000000000000;
pub(crate) const SIGN_BIT: u64 = 0x8000000000000000;
pub(crate) const TAG_NULL: u64 = 0x0001000000000000;
pub(crate) const TAG_FALSE: u64 = 0x0002000000000000;
pub(crate) const TAG_TRUE: u64 = 0x0003000000000000;
pub(crate) const TAG_INT: u64 = 0x0004000000000000;
pub(crate) const TAG_DEOPT: u64 = 0x0007000000000000;

// 48-bit signed integer payload range (payload is bits 0..47)
pub(crate) const INT48_MAX: i64 =  0x0000_7FFF_FFFF_FFFF;
pub(crate) const INT48_MIN: i64 = -0x0000_8000_0000_0000;

#[repr(transparent)]
pub struct Value(pub u64);

impl Value {
    #[inline(always)]
    pub fn null() -> Self { Value(QNAN | TAG_NULL) }
    #[inline(always)]
    pub fn bool(b: bool) -> Self {
        if b { Value(QNAN | TAG_TRUE) }
        else { Value(QNAN | TAG_FALSE) }
    }
    #[inline(always)]
    pub fn float(f: f64) -> Self {
        Value(f.to_bits())
    }
    #[inline(always)]
    pub fn int(i: i64) -> Self {
        if i >= INT48_MIN && i <= INT48_MAX {
            // Fast path: fits in the 48-bit NaN-box payload
            Value(QNAN | TAG_INT | (i as u64 & 0x0000FFFFFFFFFFFF))
        } else {
            // Overflow: promote to f64 transparently (same as V8 Smi overflow)
            Value::float(i as f64)
        }
    }
    #[inline(always)]
    pub fn obj(obj: Arc<Obj>) -> Self {
        let ptr = Arc::into_raw(obj) as u64;
        Value(SIGN_BIT | QNAN | ptr)
    }

    #[inline(always)]
    pub fn is_float(&self) -> bool { (self.0 & QNAN) != QNAN }
    #[inline(always)]
    pub fn is_null(&self) -> bool { self.0 == (QNAN | TAG_NULL) }
    #[inline(always)]
    pub fn is_bool(&self) -> bool { self.0 == (QNAN | TAG_FALSE) || self.0 == (QNAN | TAG_TRUE) }
    #[inline(always)]
    pub fn is_int(&self) -> bool { (self.0 & 0xFFFF000000000000) == (QNAN | TAG_INT) }
    #[inline(always)]
    pub fn is_obj(&self) -> bool { (self.0 & (SIGN_BIT | QNAN)) == (SIGN_BIT | QNAN) }
    #[inline(always)]
    pub fn is_deopt(&self) -> bool { (self.0 & 0xFFFF000000000000) == (QNAN | TAG_DEOPT) }

    #[inline(always)]
    pub fn deopt(ip: usize) -> Self {
        Value(QNAN | TAG_DEOPT | (ip as u64 & 0x0000FFFFFFFFFFFF))
    }

    #[inline(always)]
    pub fn as_deopt(&self) -> usize {
        (self.0 & 0x0000FFFFFFFFFFFF) as usize
    }

    #[inline(always)]
    pub fn as_float(&self) -> f64 { f64::from_bits(self.0) }
    #[inline(always)]
    pub fn as_bool(&self) -> bool { self.0 == (QNAN | TAG_TRUE) }
    #[inline(always)]
    pub fn as_int(&self) -> i64 {
        let bits = self.0 & 0x0000FFFFFFFFFFFF;
        // Sign extend from 48 bits if necessary.
        if bits & 0x0000800000000000 != 0 {
            (bits | 0xFFFF000000000000) as i64
        } else {
            bits as i64
        }
    }
    #[inline(always)]
    pub fn as_obj(&self) -> Arc<Obj> {
        let ptr = (self.0 & ! (SIGN_BIT | QNAN)) as *const Obj;
        unsafe { 
            let rc = Arc::from_raw(ptr);
            let cloned = Arc::clone(&rc);
            std::mem::forget(rc); // Don't decrement count here
            cloned
        }
    }

    /// Increment Arc refcount without creating a new Value.
    #[inline(always)]
    pub fn mark(&self) {
        if self.is_obj() {
            let ptr = (self.0 & ! (SIGN_BIT | QNAN)) as *const Obj;
            unsafe { Arc::increment_strong_count(ptr); }
        }
    }

    /// Decrement Arc refcount without dropping the Value.
    #[inline(always)]
    pub fn unmark(&self) {
        if self.is_obj() {
            let ptr = (self.0 & ! (SIGN_BIT | QNAN)) as *const Obj;
            unsafe { Arc::decrement_strong_count(ptr); }
        }
    }
}

impl Clone for Value {
    #[inline(always)]
    fn clone(&self) -> Self {
        // Fast path: only obj values (top 2 bits = 11) need refcount
        if (self.0 & (SIGN_BIT | QNAN)) == (SIGN_BIT | QNAN) {
            let ptr = (self.0 & !(SIGN_BIT | QNAN)) as *const Obj;
            unsafe { Arc::increment_strong_count(ptr); }
        }
        Value(self.0)
    }
}

impl Drop for Value {
    #[inline(always)]
    fn drop(&mut self) {
        // Fast path: only obj values (top 2 bits = 11) need refcount
        if (self.0 & (SIGN_BIT | QNAN)) == (SIGN_BIT | QNAN) {
            let ptr = (self.0 & !(SIGN_BIT | QNAN)) as *const Obj;
            unsafe { Arc::decrement_strong_count(ptr); }
        }
    }
}

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
