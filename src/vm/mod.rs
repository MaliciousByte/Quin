mod exec;
mod calls;
mod modules;
mod upvalues;
mod helpers;
pub mod interner;
pub mod obj;

use std::collections::HashMap;
use std::sync::Arc;
use std::cell::RefCell;
use crate::value::{Value, Function, Closure, Shape};
use obj::Obj;
use crate::jit::JitEngine;
use interner::StringInterner;
use std::path::PathBuf;

pub use modules::ModuleState;

const STACK_MAX: usize = 256;

pub(crate) struct CallFrame {
    pub(crate) closure: Arc<Closure>,
    pub(crate) ip: usize,
    pub(crate) stack_offset: usize,
    pub(crate) register_count: usize,
    pub(crate) dst: Option<u8>,
}

pub(crate) struct ExceptionHandler {
    pub(crate) frame_idx: usize,
    pub(crate) stack_idx: usize,
    pub(crate) catch_ip: usize,
    pub(crate) catch_reg: usize,
}

pub struct VM {
    pub(crate) frames: Vec<CallFrame>,
    pub(crate) stack: Vec<Value>,
    pub globals: HashMap<Arc<str>, Value>,
    pub(crate) handlers: Vec<ExceptionHandler>,
    pub(crate) open_upvalues: Vec<Arc<RefCell<crate::value::Upvalue>>>,
    pub(crate) root_shape: Arc<Shape>,
    pub(crate) next_shape_id: usize,
    pub jit_engine: JitEngine,
    pub interner: StringInterner,
    pub module_states: HashMap<Arc<str>, ModuleState>,
    pub script_dir: Option<PathBuf>,
    pub(crate) jit_recursion_depth: usize,
}

impl VM {
    pub fn new() -> Self {
        let mut vm = VM {
            frames: Vec::new(),
            stack: Vec::with_capacity(STACK_MAX),
            globals: HashMap::new(),
            handlers: Vec::new(),
            open_upvalues: Vec::new(),
            root_shape: Arc::new(Shape::new(0)),
            next_shape_id: 1,
            jit_engine: JitEngine::new(),
            interner: StringInterner::new(),
            module_states: HashMap::new(),
            script_dir: None,
            jit_recursion_depth: 0,
        };

        // Register core standard library globals
        crate::stdlib::register_core(&mut vm);
        
        vm
    }

    pub fn interpret(&mut self, mut function: Function) -> Result<(), String> {
        self.intern_constants(&mut function);
        let closure = Arc::new(Closure {
            function: Arc::new(function),
            upvalues: Vec::new(),
        });
        self.stack.push(Value::obj(Arc::new(Obj::Closure(closure.clone()))));
        self.call_closure(closure, 0, 0, None)?;

        self.run()
    }

    pub(crate) fn intern_constants(&mut self, function: &mut Function) {
        for i in 0..function.chunk.constants.len() {
            let val = &function.chunk.constants[i];
            if val.is_obj() {
                let obj = val.as_obj();
                match &*obj {
                    Obj::String(s) => {
                        let interned = self.interner.intern(s);
                        function.chunk.constants[i] = Value::obj(Arc::new(Obj::String(interned)));
                    }
                    Obj::Function(f) => {
                        // Cast to mut to intern its constants too
                        let f_ptr = Arc::as_ptr(f) as *mut Function;
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
                                    new_elements.push(Value::obj(Arc::new(Obj::String(self.intern(s)))));
                                    continue;
                                }
                            }
                            new_elements.push(elem.clone());
                        }
                        function.chunk.constants[i] = Value::obj(Arc::new(Obj::Tuple(new_elements)));
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn run(&mut self) -> Result<(), String> {
        let starting_depth = self.frames.len();
        loop {
            let inst = self.read_instruction_u32()?;
            let (op_byte, _, _, _) = crate::frontend::chunk::decode_inst(inst);
            exec::DISPATCH_TABLE[op_byte as usize](self, inst)?;
            if self.frames.len() < starting_depth {
                return Ok(());
            }
        }
    }
}
