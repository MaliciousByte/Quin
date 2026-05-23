use std::sync::Arc;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use crate::value::{Function, Instance, ClassValue, InstanceValue, Closure, BoundMethodValue, Value};

pub enum Obj {
    String(Arc<str>),
    Function(Arc<Function>),
    NativeFn(fn(&mut crate::vm::VM, &[Value]) -> Result<Value, String>),
    Array(RefCell<Vec<Value>>),
    Dict(RefCell<HashMap<Value, Value>>),
    Tuple(Vec<Value>),
    Set(RefCell<HashSet<Value>>),
    Instance(Arc<RefCell<Instance>>),
    Class(Arc<ClassValue>),
    Object(Arc<RefCell<InstanceValue>>),
    Closure(Arc<Closure>),
    BoundMethod(Arc<BoundMethodValue>),
}

impl Obj {
    pub fn type_name(&self) -> &'static str {
        match self {
            Obj::String(_) => "str",
            Obj::Function(_) => "task",
            Obj::NativeFn(_) => "native_task",
            Obj::Array(_) => "array",
            Obj::Dict(_) => "dict",
            Obj::Tuple(_) => "tuple",
            Obj::Set(_) => "set",
            Obj::Instance(_) => "instance",
            Obj::Class(_) => "class",
            Obj::Object(_) => "object",
            Obj::Closure(_) => "closure",
            Obj::BoundMethod(_) => "bound_method",
        }
    }
}
