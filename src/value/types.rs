use std::sync::Arc;
use std::cell::RefCell;
use std::collections::HashMap;
use super::Value;

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
