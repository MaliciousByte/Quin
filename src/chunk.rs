use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCode {
    Constant(usize),
    Null,
    True,
    False,
    Pop,
    Dup,
    
    GetLocal(usize),
    SetLocal(usize),
    GetGlobal(usize), // Simplified, usually strings
    SetGlobal(usize),
    DefineGlobal(usize),

    Equal,
    Greater,
    Less,
    Add,
    Subtract,
    Multiply,
    Divide,
    Not,
    Negate,

    JumpIfFalse(usize),
    Jump(usize),
    Loop(usize),

    Call(u8),
    Return,
    
    // Arrays
    BuildArray(u8),
    BuildDict(u8),
    BuildTuple(u8),
    BuildSet(u8),
    GetIndex,
    SetIndex,

    // Struct / Class
    BuildInstance(usize, u8), // name index, fields count
    GetProperty(usize),       // name index
    SetProperty(usize),
    
    // Other
    Throw,
    Await,
    Cast(usize), // Type name constant
    Finally,
    BuildClass(usize),
    Method(usize),
    JumpIfNull(usize),
}

#[derive(Clone, Default)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
    pub lines: Vec<usize>,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
        }
    }

    pub fn write(&mut self, op: OpCode, line: usize) {
        self.code.push(op);
        self.lines.push(line);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}
