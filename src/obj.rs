use std::rc::Rc;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use crate::value::{Function, Instance, ClassValue, InstanceValue, Closure, BoundMethodValue, Value};

pub enum Obj {
    String(Rc<str>),
    Function(Rc<Function>),
    NativeFn(fn(&[Value]) -> Result<Value, String>),
    Array(RefCell<Vec<Value>>),
    Dict(RefCell<HashMap<Value, Value>>),
    Tuple(Vec<Value>),
    Set(RefCell<HashSet<Value>>),
    Instance(Rc<RefCell<Instance>>),
    Class(Rc<ClassValue>),
    Object(Rc<RefCell<InstanceValue>>),
    Closure(Rc<Closure>),
    BoundMethod(Rc<BoundMethodValue>),
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
