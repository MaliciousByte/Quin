use crate::value::Value;
use std::cell::RefCell;
use std::sync::atomic::AtomicU32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTag {
    Int,
    Float,
    Bool,
    String,
    Nil,
}

#[derive(Debug, Clone, Copy)]
pub struct InlineCache {
    pub shape_id: u64,
    pub offset: u32,
}

pub const OP_LOAD_CONST: u8 = 0;
pub const OP_LOAD_NULL: u8 = 1;
pub const OP_LOAD_TRUE: u8 = 2;
pub const OP_LOAD_FALSE: u8 = 3;
pub const OP_MOVE: u8 = 4;
pub const OP_GET_GLOBAL: u8 = 5;
pub const OP_SET_GLOBAL: u8 = 6;
pub const OP_DEFINE_GLOBAL: u8 = 7;
pub const OP_EQUAL: u8 = 8;
pub const OP_GREATER: u8 = 9;
pub const OP_LESS: u8 = 10;
pub const OP_ADD: u8 = 11;
pub const OP_SUBTRACT: u8 = 12;
pub const OP_MULTIPLY: u8 = 13;
pub const OP_DIVIDE: u8 = 14;
pub const OP_NOT: u8 = 15;
pub const OP_NEGATE: u8 = 16;
pub const OP_JUMP_IF_FALSE: u8 = 17;
pub const OP_JUMP: u8 = 18;
pub const OP_LOOP: u8 = 19;
pub const OP_CALL: u8 = 20;
pub const OP_RETURN: u8 = 21;
pub const OP_BUILD_ARRAY: u8 = 22;
pub const OP_BUILD_DICT: u8 = 23;
pub const OP_BUILD_TUPLE: u8 = 24;
pub const OP_BUILD_SET: u8 = 25;
pub const OP_GET_INDEX: u8 = 26;
pub const OP_SET_INDEX: u8 = 27;
pub const OP_BUILD_INSTANCE: u8 = 28;
pub const OP_GET_PROPERTY: u8 = 29;
pub const OP_SET_PROPERTY: u8 = 30;
pub const OP_THROW: u8 = 31;
pub const OP_BUILD_CLASS: u8 = 32;
pub const OP_METHOD: u8 = 33;
pub const OP_JUMP_IF_NULL: u8 = 34;
pub const OP_CLOSURE: u8 = 35;
pub const OP_GET_UPVALUE: u8 = 36;
pub const OP_SET_UPVALUE: u8 = 37;
pub const OP_CLOSE_UPVALUE: u8 = 38;
pub const OP_SETUP_HANDLER: u8 = 39;
pub const OP_POP_HANDLER: u8 = 40;
pub const OP_IMPORT_MODULE: u8 = 41;
pub const OP_IMPORT_ITEMS: u8 = 42;
pub const OP_NEQ: u8 = 43;

// Specialized opcodes
pub const OP_ADD_INT: u8 = 100;
pub const OP_SUB_INT: u8 = 101;
pub const OP_MUL_INT: u8 = 102;
pub const OP_DIV_INT: u8 = 103;
pub const OP_LT_INT: u8 = 104;
pub const OP_GT_INT: u8 = 105;
pub const OP_EQ_INT: u8 = 106;
pub const OP_NEQ_INT: u8 = 107;

pub const OP_ADD_FLOAT: u8 = 108;
pub const OP_SUB_FLOAT: u8 = 109;
pub const OP_MUL_FLOAT: u8 = 110;
pub const OP_DIV_FLOAT: u8 = 111;
pub const OP_LT_FLOAT: u8 = 112;
pub const OP_GT_FLOAT: u8 = 113;
pub const OP_EQ_FLOAT: u8 = 114;
pub const OP_NEQ_FLOAT: u8 = 115;

pub const OP_GET_PROPERTY_CACHED: u8 = 116;

#[inline(always)]
pub fn encode_inst(op: u8, dst: u8, src1: u8, src2: u8) -> u32 {
    ((op as u32) << 24) | ((dst as u32) << 16) | ((src1 as u32) << 8) | (src2 as u32)
}

#[inline(always)]
pub fn encode_inst_imm16(op: u8, dst: u8, imm: u16) -> u32 {
    ((op as u32) << 24) | ((dst as u32) << 16) | (imm as u32)
}

#[inline(always)]
pub fn decode_inst(inst: u32) -> (u8, u8, u8, u8) {
    (
        (inst >> 24) as u8,
        ((inst >> 16) & 0xff) as u8,
        ((inst >> 8) & 0xff) as u8,
        (inst & 0xff) as u8,
    )
}

#[inline(always)]
pub fn decode_inst_imm16(inst: u32) -> (u8, u8, u16) {
    (
        (inst >> 24) as u8,
        ((inst >> 16) & 0xff) as u8,
        (inst & 0xffff) as u16,
    )
}

pub struct Chunk {
    pub code: Vec<u32>,                          // register instructions
    pub constants: Vec<Value>,
    pub lines: Vec<usize>,
    pub register_count: u32,
    pub type_metadata: Vec<Option<TypeTag>>,     // per register, from annotations
    pub mutability_flags: Vec<bool>,             // per register, from let/let mut
    pub profiling_counter: AtomicU32,
    pub observed_types: Vec<Option<TypeTag>>,    // filled at runtime
    pub bytecode_hash: [u8; 16],                 // computed once at compile time, immutable
    pub inline_caches: RefCell<Vec<Option<InlineCache>>>,
}

impl Clone for Chunk {
    fn clone(&self) -> Self {
        Chunk {
            code: self.code.clone(),
            constants: self.constants.clone(),
            lines: self.lines.clone(),
            register_count: self.register_count,
            type_metadata: self.type_metadata.clone(),
            mutability_flags: self.mutability_flags.clone(),
            profiling_counter: AtomicU32::new(self.profiling_counter.load(std::sync::atomic::Ordering::Relaxed)),
            observed_types: self.observed_types.clone(),
            bytecode_hash: self.bytecode_hash,
            inline_caches: RefCell::new(self.inline_caches.borrow().clone()),
        }
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
            register_count: 0,
            type_metadata: Vec::new(),
            mutability_flags: Vec::new(),
            profiling_counter: AtomicU32::new(0),
            observed_types: Vec::new(),
            bytecode_hash: [0; 16],
            inline_caches: RefCell::new(Vec::new()),
        }
    }
}

impl Chunk {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(&mut self, op: u32, line: usize) {
        self.code.push(op);
        self.lines.push(line);
        self.inline_caches.borrow_mut().push(None); // Initialize cache slot
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}
