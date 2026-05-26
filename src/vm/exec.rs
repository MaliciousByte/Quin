use std::collections::HashMap;
use std::sync::Arc;
use std::cell::RefCell;
use crate::frontend::chunk::{
    decode_inst, decode_inst_imm16, encode_inst, InlineCache,
    OP_LOAD_CONST, OP_LOAD_NULL, OP_LOAD_TRUE, OP_LOAD_FALSE, OP_MOVE,
    OP_GET_GLOBAL, OP_SET_GLOBAL, OP_DEFINE_GLOBAL, OP_EQUAL, OP_GREATER,
    OP_LESS, OP_ADD, OP_SUBTRACT, OP_MULTIPLY, OP_DIVIDE, OP_NOT, OP_NEGATE,
    OP_JUMP_IF_FALSE, OP_JUMP, OP_LOOP, OP_CALL, OP_RETURN, OP_BUILD_ARRAY,
    OP_BUILD_DICT, OP_BUILD_TUPLE, OP_BUILD_SET, OP_GET_INDEX, OP_SET_INDEX,
    OP_BUILD_INSTANCE, OP_GET_PROPERTY, OP_SET_PROPERTY, OP_THROW, OP_BUILD_CLASS,
    OP_METHOD, OP_JUMP_IF_NULL, OP_CLOSURE, OP_GET_UPVALUE, OP_SET_UPVALUE,
    OP_CLOSE_UPVALUE, OP_SETUP_HANDLER, OP_POP_HANDLER, OP_IMPORT_MODULE,
    OP_IMPORT_ITEMS, OP_NEQ,
    OP_ADD_INT, OP_SUB_INT, OP_MUL_INT, OP_DIV_INT, OP_LT_INT, OP_GT_INT, OP_EQ_INT, OP_NEQ_INT,
    OP_ADD_FLOAT, OP_SUB_FLOAT, OP_MUL_FLOAT, OP_DIV_FLOAT, OP_LT_FLOAT, OP_GT_FLOAT, OP_EQ_FLOAT, OP_NEQ_FLOAT,
    OP_GET_PROPERTY_CACHED,
};
use crate::value::{Value, Closure};
use crate::vm::obj::Obj;
use super::{VM, CallFrame};

type Handler = fn(&mut VM, u32) -> Result<(), String>;

pub(crate) static DISPATCH_TABLE: [Handler; 256] = init_dispatch_table();

const fn init_dispatch_table() -> [Handler; 256] {
    let mut table = [handle_unknown as Handler; 256];
    table[OP_LOAD_CONST as usize] = handle_load_const;
    table[OP_LOAD_NULL as usize] = handle_load_null;
    table[OP_LOAD_TRUE as usize] = handle_load_true;
    table[OP_LOAD_FALSE as usize] = handle_load_false;
    table[OP_MOVE as usize] = handle_move;
    table[OP_GET_GLOBAL as usize] = handle_get_global;
    table[OP_SET_GLOBAL as usize] = handle_set_global;
    table[OP_DEFINE_GLOBAL as usize] = handle_define_global;
    table[OP_EQUAL as usize] = handle_equal;
    table[OP_GREATER as usize] = handle_greater;
    table[OP_LESS as usize] = handle_less;
    table[OP_ADD as usize] = handle_add;
    table[OP_SUBTRACT as usize] = handle_subtract;
    table[OP_MULTIPLY as usize] = handle_multiply;
    table[OP_DIVIDE as usize] = handle_divide;
    table[OP_NOT as usize] = handle_not;
    table[OP_NEGATE as usize] = handle_negate;
    table[OP_JUMP_IF_FALSE as usize] = handle_jump_if_false;
    table[OP_JUMP as usize] = handle_jump;
    table[OP_LOOP as usize] = handle_loop;
    table[OP_CALL as usize] = handle_call;
    table[OP_RETURN as usize] = handle_return;
    table[OP_BUILD_ARRAY as usize] = handle_build_array;
    table[OP_BUILD_DICT as usize] = handle_build_dict;
    table[OP_BUILD_TUPLE as usize] = handle_build_tuple;
    table[OP_BUILD_SET as usize] = handle_build_set;
    table[OP_GET_INDEX as usize] = handle_get_index;
    table[OP_SET_INDEX as usize] = handle_set_index;
    table[OP_BUILD_INSTANCE as usize] = handle_build_instance;
    table[OP_GET_PROPERTY as usize] = handle_get_property;
    table[OP_SET_PROPERTY as usize] = handle_set_property;
    table[OP_THROW as usize] = handle_throw;
    table[OP_BUILD_CLASS as usize] = handle_build_class;
    table[OP_METHOD as usize] = handle_method;
    table[OP_JUMP_IF_NULL as usize] = handle_jump_if_null;
    table[OP_CLOSURE as usize] = handle_closure;
    table[OP_GET_UPVALUE as usize] = handle_get_upvalue;
    table[OP_SET_UPVALUE as usize] = handle_set_upvalue;
    table[OP_CLOSE_UPVALUE as usize] = handle_close_upvalue;
    table[OP_SETUP_HANDLER as usize] = handle_setup_handler;
    table[OP_POP_HANDLER as usize] = handle_pop_handler;
    table[OP_IMPORT_MODULE as usize] = handle_import_module;
    table[OP_IMPORT_ITEMS as usize] = handle_import_items;
    table[OP_NEQ as usize] = handle_neq;

    // Specialized opcodes
    table[OP_ADD_INT as usize] = handle_add_int;
    table[OP_SUB_INT as usize] = handle_sub_int;
    table[OP_MUL_INT as usize] = handle_mul_int;
    table[OP_DIV_INT as usize] = handle_div_int;
    table[OP_LT_INT as usize] = handle_lt_int;
    table[OP_GT_INT as usize] = handle_gt_int;
    table[OP_EQ_INT as usize] = handle_eq_int;
    table[OP_NEQ_INT as usize] = handle_neq_int;
    table[OP_ADD_FLOAT as usize] = handle_add_float;
    table[OP_SUB_FLOAT as usize] = handle_sub_float;
    table[OP_MUL_FLOAT as usize] = handle_mul_float;
    table[OP_DIV_FLOAT as usize] = handle_div_float;
    table[OP_LT_FLOAT as usize] = handle_lt_float;
    table[OP_GT_FLOAT as usize] = handle_gt_float;
    table[OP_EQ_FLOAT as usize] = handle_eq_float;
    table[OP_NEQ_FLOAT as usize] = handle_neq_float;
    table[OP_GET_PROPERTY_CACHED as usize] = handle_get_property_cached;

    table
}

fn handle_unknown(_vm: &mut VM, inst: u32) -> Result<(), String> {
    let (op, _, _, _) = decode_inst(inst);
    Err(format!("Unknown or unimplemented opcode: {}", op))
}

fn handle_load_const(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, const_idx) = decode_inst_imm16(inst);
    let constant = vm.read_constant(const_idx as usize)?;
    let offset = vm.current_frame()?.stack_offset;
    vm.stack[offset + dst as usize] = constant;
    Ok(())
}

fn handle_load_null(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, _, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    vm.stack[offset + dst as usize] = Value::null();
    Ok(())
}

fn handle_load_true(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, _, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    vm.stack[offset + dst as usize] = Value::bool(true);
    Ok(())
}

fn handle_load_false(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, _, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    vm.stack[offset + dst as usize] = Value::bool(false);
    Ok(())
}

fn handle_move(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    vm.stack[offset + dst as usize] = vm.stack[offset + src as usize].clone();
    Ok(())
}

fn handle_get_global(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, const_idx) = decode_inst_imm16(inst);
    let name = vm.read_string(const_idx as usize)?;
    let val = vm.globals.get(&name).cloned().ok_or_else(|| format!("Undefined variable '{}'.", name))?;
    let offset = vm.current_frame()?.stack_offset;
    vm.stack[offset + dst as usize] = val;
    Ok(())
}

fn handle_set_global(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, src, const_idx) = decode_inst_imm16(inst);
    let name = vm.read_string(const_idx as usize)?;
    let offset = vm.current_frame()?.stack_offset;
    let val = vm.stack[offset + src as usize].clone();
    if vm.globals.contains_key(&name) {
        vm.globals.insert(name, val);
        Ok(())
    } else {
        Err(format!("Undefined variable '{}'.", name))
    }
}

fn handle_define_global(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, src, const_idx) = decode_inst_imm16(inst);
    let name = vm.read_string(const_idx as usize)?;
    let offset = vm.current_frame()?.stack_offset;
    let val = vm.stack[offset + src as usize].clone();
    vm.globals.insert(name, val);
    Ok(())
}

fn handle_equal(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    vm.stack[offset + dst as usize] = Value::bool(a == b);
    Ok(())
}

fn handle_neq(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    vm.stack[offset + dst as usize] = Value::bool(a != b);
    Ok(())
}

fn handle_greater(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    let res = if a.is_int() && b.is_int() {
        a.as_int() > b.as_int()
    } else if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        va > vb
    } else {
        return Err("Operands must be numbers.".to_string());
    };
    vm.stack[offset + dst as usize] = Value::bool(res);
    Ok(())
}

fn handle_less(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    let res = if a.is_int() && b.is_int() {
        a.as_int() < b.as_int()
    } else if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        va < vb
    } else {
        return Err("Operands must be numbers.".to_string());
    };
    vm.stack[offset + dst as usize] = Value::bool(res);
    Ok(())
}

fn handle_add(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_ADD_INT, dst, src1, src2); }
        vm.stack[offset + dst as usize] = Value::int(a.as_int() + b.as_int());
    } else if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_ADD_FLOAT, dst, src1, src2); }
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::float(va + vb);
    } else if a.is_obj() || b.is_obj() {
        let res = format!("{}{}", a, b);
        let interned = vm.intern(&res);
        vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::String(interned)));
    } else {
        return Err("Operands must be two numbers or include a string.".to_string());
    }
    Ok(())
}

fn handle_subtract(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_SUB_INT, dst, src1, src2); }
        vm.stack[offset + dst as usize] = Value::int(a.as_int() - b.as_int());
    } else if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_SUB_FLOAT, dst, src1, src2); }
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::float(va - vb);
    } else {
        return Err("Operands must be numbers.".to_string());
    }
    Ok(())
}

fn handle_multiply(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_MUL_INT, dst, src1, src2); }
        vm.stack[offset + dst as usize] = Value::int(a.as_int() * b.as_int());
    } else if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_MUL_FLOAT, dst, src1, src2); }
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::float(va * vb);
    } else {
        return Err("Operands must be numbers.".to_string());
    }
    Ok(())
}

fn handle_divide(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_DIV_INT, dst, src1, src2); }
        vm.stack[offset + dst as usize] = Value::int(a.as_int() / b.as_int());
    } else if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_DIV_FLOAT, dst, src1, src2); }
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::float(va / vb);
    } else {
        return Err("Operands must be numbers.".to_string());
    }
    Ok(())
}

fn handle_add_int(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        vm.stack[offset + dst as usize] = Value::int(a.as_int() + b.as_int());
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_ADD, dst, src1, src2); }
        handle_add(vm, inst)
    }
}

fn handle_sub_int(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        vm.stack[offset + dst as usize] = Value::int(a.as_int() - b.as_int());
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_SUBTRACT, dst, src1, src2); }
        handle_subtract(vm, inst)
    }
}

fn handle_mul_int(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        vm.stack[offset + dst as usize] = Value::int(a.as_int() * b.as_int());
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_MULTIPLY, dst, src1, src2); }
        handle_multiply(vm, inst)
    }
}

fn handle_div_int(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        vm.stack[offset + dst as usize] = Value::int(a.as_int() / b.as_int());
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_DIVIDE, dst, src1, src2); }
        handle_divide(vm, inst)
    }
}

fn handle_add_float(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::float(va + vb);
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_ADD, dst, src1, src2); }
        handle_add(vm, inst)
    }
}

fn handle_sub_float(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::float(va - vb);
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_SUBTRACT, dst, src1, src2); }
        handle_subtract(vm, inst)
    }
}

fn handle_mul_float(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::float(va * vb);
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_MULTIPLY, dst, src1, src2); }
        handle_multiply(vm, inst)
    }
}

fn handle_div_float(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::float(va / vb);
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_DIVIDE, dst, src1, src2); }
        handle_divide(vm, inst)
    }
}

fn handle_lt_int(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        vm.stack[offset + dst as usize] = Value::bool(a.as_int() < b.as_int());
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_LESS, dst, src1, src2); }
        handle_less(vm, inst)
    }
}

fn handle_gt_int(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        vm.stack[offset + dst as usize] = Value::bool(a.as_int() > b.as_int());
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_GREATER, dst, src1, src2); }
        handle_greater(vm, inst)
    }
}

fn handle_eq_int(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        vm.stack[offset + dst as usize] = Value::bool(a.as_int() == b.as_int());
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_EQUAL, dst, src1, src2); }
        handle_equal(vm, inst)
    }
}

fn handle_neq_int(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if a.is_int() && b.is_int() {
        vm.stack[offset + dst as usize] = Value::bool(a.as_int() != b.as_int());
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_NEQ, dst, src1, src2); }
        handle_neq(vm, inst)
    }
}

fn handle_lt_float(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::bool(va < vb);
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_LESS, dst, src1, src2); }
        handle_less(vm, inst)
    }
}

fn handle_gt_float(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::bool(va > vb);
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_GREATER, dst, src1, src2); }
        handle_greater(vm, inst)
    }
}

fn handle_eq_float(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::bool(va == vb);
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_EQUAL, dst, src1, src2); }
        handle_equal(vm, inst)
    }
}

fn handle_neq_float(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src1, src2) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let a = &vm.stack[offset + src1 as usize];
    let b = &vm.stack[offset + src2 as usize];
    if (a.is_float() || a.is_int()) && (b.is_float() || b.is_int()) {
        let va = if a.is_int() { a.as_int() as f64 } else { a.as_float() };
        let vb = if b.is_int() { b.as_int() as f64 } else { b.as_float() };
        vm.stack[offset + dst as usize] = Value::bool(va != vb);
        Ok(())
    } else {
        let ip = vm.current_frame()?.ip - 1;
        let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
        unsafe { *code_ptr.add(ip) = encode_inst(OP_NEQ, dst, src1, src2); }
        handle_neq(vm, inst)
    }
}

fn handle_not(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let val = &vm.stack[offset + src as usize];
    let res = vm.is_falsey(val);
    vm.stack[offset + dst as usize] = Value::bool(res);
    Ok(())
}

fn handle_negate(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, src, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let val = &vm.stack[offset + src as usize];
    let res = if val.is_int() {
        Value::int(-val.as_int())
    } else if val.is_float() {
        Value::float(-val.as_float())
    } else {
        return Err("Operand must be a number.".to_string());
    };
    vm.stack[offset + dst as usize] = res;
    Ok(())
}

fn handle_jump_if_false(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, src, offset) = decode_inst_imm16(inst);
    let stack_offset = vm.current_frame()?.stack_offset;
    let val = &vm.stack[stack_offset + src as usize];
    let is_false = vm.is_falsey(val);
    if is_false {
        vm.current_frame_mut()?.ip += offset as usize;
    }
    Ok(())
}

fn handle_jump(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, _, offset) = decode_inst_imm16(inst);
    let frame = vm.current_frame_mut()?;
    frame.ip += offset as usize;
    Ok(())
}

fn handle_loop(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, _, offset) = decode_inst_imm16(inst);
    let is_hot = {
        let frame = vm.current_frame_mut()?;
        frame.ip -= offset as usize;
        let counter = frame.closure.function.chunk.profiling_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if counter >= 1000 && !frame.closure.function.is_hot.load(std::sync::atomic::Ordering::Relaxed) {
            frame.closure.function.is_hot.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        frame.closure.function.is_hot.load(std::sync::atomic::Ordering::Relaxed)
    };

    if is_hot {
        let closure = vm.current_frame()?.closure.clone();
        let native_ptr = vm.jit_engine.compile(&closure.function);

        if !native_ptr.is_null() && vm.jit_recursion_depth < 500 {
            closure.function.native_ptr.store(
                native_ptr as *mut u8,
                std::sync::atomic::Ordering::Relaxed,
            );

            // OSR: pop interpreter frame, restart function in JIT from IP=0.
            let frame = vm.frames.pop().unwrap();
            let stack_offset = frame.stack_offset;

            let needed = stack_offset + closure.function.max_locals;
            if needed > super::STACK_MAX {
                return Err("Stack overflow".into());
            }
            if vm.stack.len() < needed {
                vm.stack.resize(needed, Value::null());
            }

            let native_fn: extern "C" fn(*mut VM, *const Value) -> Value =
                unsafe { std::mem::transmute(native_ptr) };
            let args_ptr = unsafe { vm.stack.as_ptr().add(stack_offset) };
            vm.jit_recursion_depth += 1;
            let result = native_fn(vm as *mut VM, args_ptr);
            vm.jit_recursion_depth -= 1;

            if result.is_deopt() {
                // Type guard failed — resume interpreter at deopt IP.
                let deopt_ip = result.as_deopt();
                let frame = CallFrame {
                    closure,
                    ip: deopt_ip,
                    stack_offset,
                    register_count: needed - stack_offset,
                    dst: frame.dst,
                };
                vm.frames.push(frame);
            } else {
                // JIT finished — write/push result and return to caller.
                if let Some(dst_reg) = frame.dst {
                    let caller_frame = vm.frames.last().ok_or("No caller frame found.")?;
                    let caller_offset = caller_frame.stack_offset;
                    let caller_reg_count = caller_frame.register_count;
                    vm.stack.truncate(caller_offset + caller_reg_count);
                    vm.stack[caller_offset + dst_reg as usize] = result;
                } else {
                    vm.stack.truncate(frame.stack_offset);
                    vm.push(result);
                }
            }
        }
    }
    Ok(())
}

fn handle_jump_if_null(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, src, offset) = decode_inst_imm16(inst);
    let stack_offset = vm.current_frame()?.stack_offset;
    let val = &vm.stack[stack_offset + src as usize];
    if val.is_null() {
        vm.current_frame_mut()?.ip += offset as usize;
    }
    Ok(())
}

fn handle_call(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, callee_reg, arg_count) = decode_inst(inst);
    vm.call_value(callee_reg, arg_count, Some(dst))
}

fn handle_return(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, src, _, _) = decode_inst(inst);
    let frame = vm.frames.pop().ok_or("No frame to return from.")?;
    let val = vm.stack[frame.stack_offset + src as usize].clone();
    vm.close_upvalues(frame.stack_offset);
    
    if let Some(dst_reg) = frame.dst {
        let caller_frame = vm.frames.last().ok_or("No caller frame found for destination register.")?;
        let caller_offset = caller_frame.stack_offset;
        let caller_reg_count = caller_frame.register_count;
        vm.stack.truncate(caller_offset + caller_reg_count);
        vm.stack[caller_offset + dst_reg as usize] = val;
    } else {
        vm.stack.truncate(frame.stack_offset);
        vm.push(val);
    }
    Ok(())
}

fn handle_build_array(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, start_reg, count) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let mut elements = Vec::with_capacity(count as usize);
    for i in 0..count {
        elements.push(vm.stack[offset + start_reg as usize + i as usize].clone());
    }
    vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::Array(RefCell::new(elements))));
    Ok(())
}

fn handle_build_dict(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, start_reg, count) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let mut map = HashMap::new();
    for i in 0..count {
        let key = vm.stack[offset + start_reg as usize + i as usize * 2].clone();
        let val = vm.stack[offset + start_reg as usize + i as usize * 2 + 1].clone();
        map.insert(key, val);
    }
    vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::Dict(RefCell::new(map))));
    Ok(())
}

fn handle_build_tuple(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, start_reg, count) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let mut elements = Vec::with_capacity(count as usize);
    for i in 0..count {
        elements.push(vm.stack[offset + start_reg as usize + i as usize].clone());
    }
    vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::Tuple(elements)));
    Ok(())
}

fn handle_build_set(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, start_reg, count) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let mut set = std::collections::HashSet::new();
    for i in 0..count {
        set.insert(vm.stack[offset + start_reg as usize + i as usize].clone());
    }
    vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::Set(RefCell::new(set))));
    Ok(())
}

fn handle_get_index(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, obj_reg, index_reg) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let target = &vm.stack[offset + obj_reg as usize];
    let index = &vm.stack[offset + index_reg as usize];
    
    let res = if target.is_obj() {
        match &*target.as_obj() {
            Obj::Array(arr) => {
                if index.is_int() {
                    let elements = arr.borrow();
                    let i = index.as_int();
                    if i >= 0 && (i as usize) < elements.len() {
                        elements[i as usize].clone()
                    } else {
                        return Err(format!("Array index out of bounds: {}", i));
                    }
                } else {
                    return Err("Array index must be an integer.".to_string());
                }
            }
            Obj::Dict(map) => {
                map.borrow().get(index).cloned().unwrap_or_else(Value::null)
            }
            Obj::Tuple(elements) => {
                if index.is_int() {
                    let i = index.as_int();
                    if i >= 0 && (i as usize) < elements.len() {
                        elements[i as usize].clone()
                    } else {
                        return Err(format!("Tuple index out of bounds: {}", i));
                    }
                } else {
                    return Err("Tuple index must be an integer.".to_string());
                }
            }
            _ => return Err("Only arrays, dicts, and tuples can be indexed.".to_string()),
        }
    } else {
        return Err("Target is not indexable.".to_string());
    };
    vm.stack[offset + dst as usize] = res;
    Ok(())
}

fn handle_set_index(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, obj_reg, index_reg, val_reg) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let target = &vm.stack[offset + obj_reg as usize];
    let index = &vm.stack[offset + index_reg as usize];
    let value = &vm.stack[offset + val_reg as usize];
    
    if target.is_obj() {
        match &*target.as_obj() {
            Obj::Array(arr) => {
                if index.is_int() {
                    let mut elements = arr.borrow_mut();
                    let i = index.as_int();
                    if i >= 0 && (i as usize) < elements.len() {
                        elements[i as usize] = value.clone();
                    } else {
                        return Err(format!("Array index out of bounds: {}", i));
                    }
                } else {
                    return Err("Array index must be an integer.".to_string());
                }
            }
            Obj::Dict(map) => {
                map.borrow_mut().insert(index.clone(), value.clone());
            }
            _ => return Err("Only arrays and dicts can be indexed for assignment.".to_string()),
        }
    } else {
        return Err("Target is not indexable for assignment.".to_string());
    }
    Ok(())
}

fn handle_build_instance(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, start_reg, count) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    
    let class_name_val = &vm.stack[offset + dst as usize];
    let name = if class_name_val.is_obj() {
        if let Obj::String(s) = &*class_name_val.as_obj() {
            s.clone()
        } else {
            return Err("Expected class name in destination register.".to_string());
        }
    } else {
        return Err("Expected class name string in destination register.".to_string());
    };

    let mut field_values = Vec::with_capacity(count as usize);
    let mut current_shape = vm.root_shape.clone();

    for i in 0..count as usize {
        let field_name_val = &vm.stack[offset + start_reg as usize + i * 2];
        let field_val = &vm.stack[offset + start_reg as usize + i * 2 + 1];
        
        let field_name = if field_name_val.is_obj() {
            if let Obj::String(s) = &*field_name_val.as_obj() {
                s.clone()
            } else {
                return Err("Field name must be a string.".to_string());
            }
        } else {
            return Err("Field name must be a string.".to_string());
        };
        
        // Transition shape
        let existing = current_shape.transitions.borrow().get(&field_name).cloned();
        let next_shape = if let Some(next) = existing {
            next
        } else {
            let next = current_shape.transition(field_name.clone(), vm.next_shape_id);
            vm.next_shape_id += 1;
            current_shape.transitions.borrow_mut().insert(field_name, next.clone());
            next
        };
        current_shape = next_shape;
        field_values.push(field_val.clone());
    }

    let inst_obj = crate::value::Instance {
        name,
        shape: current_shape,
        fields: field_values,
    };
    vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::Instance(Arc::new(RefCell::new(inst_obj)))));
    Ok(())
}

fn handle_get_property(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, obj_reg, name_reg) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let obj = vm.stack[offset + obj_reg as usize].clone();
    let name_val = &vm.stack[offset + name_reg as usize];
    
    let name = if name_val.is_obj() {
        if let Obj::String(s) = &*name_val.as_obj() {
            s.clone()
        } else {
            return Err("Property name must be a string.".to_string());
        }
    } else {
        return Err("Property name must be a string.".to_string());
    };

    let ip = vm.current_frame()?.ip - 1;

    if obj.is_obj() {
        match &*obj.as_obj() {
            Obj::Instance(inst_ptr) => {
                let inst_ref = inst_ptr.borrow();
                let prop_offset = if let Some(&off) = inst_ref.shape.property_offsets.get(&name) {
                    off
                } else {
                    return Err(format!("Property '{}' not found on instance.", name));
                };

                // Update inline cache
                let cache = InlineCache { shape_id: inst_ref.shape.id as u64, offset: prop_offset as u32 };
                vm.current_frame()?.closure.function.chunk.inline_caches.borrow_mut()[ip] = Some(cache);

                // Specialize instruction in-place
                let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
                unsafe { *code_ptr.add(ip) = encode_inst(OP_GET_PROPERTY_CACHED, dst, obj_reg, name_reg); }

                vm.stack[offset + dst as usize] = inst_ref.fields[prop_offset].clone();
            }
            Obj::Object(obj_val) => {
                let obj_ref = obj_val.borrow();
                if let Some(&prop_offset) = obj_ref.shape.property_offsets.get(&name) {
                    let cache = InlineCache { shape_id: obj_ref.shape.id as u64, offset: prop_offset as u32 };
                    vm.current_frame()?.closure.function.chunk.inline_caches.borrow_mut()[ip] = Some(cache);

                    let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
                    unsafe { *code_ptr.add(ip) = encode_inst(OP_GET_PROPERTY_CACHED, dst, obj_reg, name_reg); }

                    vm.stack[offset + dst as usize] = obj_ref.fields.borrow()[prop_offset].clone();
                } else {
                    // Lookup in class hierarchy
                    let mut current_class = Some(obj_ref.class.clone());
                    let mut found = false;
                    while let Some(cls) = current_class {
                        if let Some(method_val) = cls.methods.borrow().get(&name) {
                            if method_val.is_obj() {
                                match &*method_val.as_obj() {
                                    Obj::Function(method) => {
                                        vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::BoundMethod(Arc::new(crate::value::BoundMethodValue {
                                            receiver: Value::obj(Arc::new(Obj::Object(obj_val.clone()))),
                                            method: method.clone(),
                                        }))));
                                        found = true;
                                        break;
                                    }
                                    Obj::Closure(closure) => {
                                        vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::BoundMethod(Arc::new(crate::value::BoundMethodValue {
                                            receiver: Value::obj(Arc::new(Obj::Object(obj_val.clone()))),
                                            method: closure.function.clone(),
                                        }))));
                                        found = true;
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        current_class = cls.superclass.clone();
                    }
                    if !found {
                        return Err(format!("Property '{}' not found on instance of {}.", name, obj_ref.class.name));
                    }
                }
            }
            Obj::Dict(map) => {
                let key = Value::obj(Arc::new(Obj::String(name)));
                vm.stack[offset + dst as usize] = map.borrow().get(&key).cloned().unwrap_or_else(Value::null);
            }
            Obj::Class(cls_val) => {
                let mut current_class = Some(cls_val.clone());
                let mut found = false;
                while let Some(cls) = current_class {
                    if let Some(method) = cls.methods.borrow().get(&name) {
                        vm.stack[offset + dst as usize] = method.clone();
                        found = true;
                        break;
                    }
                    current_class = cls.superclass.clone();
                }
                if !found {
                    vm.stack[offset + dst as usize] = Value::null();
                }
            }
            _ => return Err("Only instances, objects, classes, and dicts have properties.".to_string()),
        }
    } else {
        return Err("Target is not an object.".to_string());
    }
    Ok(())
}

fn handle_get_property_cached(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, obj_reg, name_reg) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let obj = &vm.stack[offset + obj_reg as usize];
    let ip = vm.current_frame()?.ip - 1;

    let cache = vm.current_frame()?.closure.function.chunk.inline_caches.borrow()[ip].unwrap();

    if obj.is_obj() {
        match &*obj.as_obj() {
            Obj::Instance(inst_ptr) => {
                let inst_ref = inst_ptr.borrow();
                if inst_ref.shape.id as u64 == cache.shape_id {
                    vm.stack[offset + dst as usize] = inst_ref.fields[cache.offset as usize].clone();
                    return Ok(());
                }
            }
            Obj::Object(obj_val) => {
                let obj_ref = obj_val.borrow();
                if obj_ref.shape.id as u64 == cache.shape_id {
                    vm.stack[offset + dst as usize] = obj_ref.fields.borrow()[cache.offset as usize].clone();
                    return Ok(());
                }
            }
            _ => {}
        }
    }

    // Cache miss: de-specialize and retry slow path
    let code_ptr = vm.current_frame()?.closure.function.chunk.code.as_ptr() as *mut u32;
    unsafe { *code_ptr.add(ip) = encode_inst(OP_GET_PROPERTY, dst, obj_reg, name_reg); }
    handle_get_property(vm, inst)
}

fn handle_set_property(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, obj_reg, name_reg, val_reg) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let obj = vm.stack[offset + obj_reg as usize].clone();
    let name_val = &vm.stack[offset + name_reg as usize];
    let value = vm.stack[offset + val_reg as usize].clone();
    
    let name = if name_val.is_obj() {
        if let Obj::String(s) = &*name_val.as_obj() {
            s.clone()
        } else {
            return Err("Property name must be a string.".to_string());
        }
    } else {
        return Err("Property name must be a string.".to_string());
    };

    if obj.is_obj() {
        match &*obj.as_obj() {
            Obj::Instance(inst_ptr) => {
                let mut inst_ref = inst_ptr.borrow_mut();
                if let Some(&prop_offset) = inst_ref.shape.property_offsets.get(&name) {
                    inst_ref.fields[prop_offset] = value;
                } else {
                    // Shape transition
                    let existing = inst_ref.shape.transitions.borrow().get(&name).cloned();
                    let next_shape = if let Some(s) = existing {
                        s
                    } else {
                        let ns = inst_ref.shape.transition(name.clone(), vm.next_shape_id);
                        vm.next_shape_id += 1;
                        inst_ref.shape.transitions.borrow_mut().insert(name, ns.clone());
                        ns
                    };
                    inst_ref.shape = next_shape;
                    inst_ref.fields.push(value);
                }
            }
            Obj::Object(obj_val) => {
                let mut obj_ref = obj_val.borrow_mut();
                if let Some(&prop_offset) = obj_ref.shape.property_offsets.get(&name) {
                    obj_ref.fields.borrow_mut()[prop_offset] = value;
                } else {
                    // Shape transition
                    let existing = obj_ref.shape.transitions.borrow().get(&name).cloned();
                    let next_shape = if let Some(s) = existing {
                        s
                    } else {
                        let ns = obj_ref.shape.transition(name.clone(), vm.next_shape_id);
                        vm.next_shape_id += 1;
                        obj_ref.shape.transitions.borrow_mut().insert(name, ns.clone());
                        ns
                    };
                    obj_ref.shape = next_shape;
                    obj_ref.fields.borrow_mut().push(value);
                }
            }
            Obj::Dict(map) => {
                let key = Value::obj(Arc::new(Obj::String(name)));
                map.borrow_mut().insert(key, value);
            }
            Obj::Class(cls_val) => {
                cls_val.methods.borrow_mut().insert(name, value);
            }
            _ => return Err("Only instances, objects, classes, and dicts have properties.".to_string()),
        }
    } else {
        return Err("Target is not an object.".to_string());
    }
    Ok(())
}

fn handle_throw(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, src, _, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let error = vm.stack[offset + src as usize].clone();
    vm.handle_exception(error)?;
    Ok(())
}

fn handle_build_class(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, super_reg, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let class_name_val = &vm.stack[offset + dst as usize];
    let name = if class_name_val.is_obj() {
        if let Obj::String(s) = &*class_name_val.as_obj() {
            s.clone()
        } else {
            return Err("Expected class name string in destination register.".to_string());
        }
    } else {
        return Err("Expected class name string in destination register.".to_string());
    };

    let super_val = &vm.stack[offset + super_reg as usize];
    let mut superclass = None;
    if super_val.is_obj() {
        if let Obj::Class(cls) = &*super_val.as_obj() {
            superclass = Some(cls.clone());
        }
    }

    let cls = Arc::new(crate::value::ClassValue {
        name,
        superclass,
        methods: RefCell::new(HashMap::new()),
    });
    vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::Class(cls)));
    Ok(())
}

fn handle_method(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, cls_reg, name_reg, method_reg) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let class_val = &vm.stack[offset + cls_reg as usize];
    let name_val = &vm.stack[offset + name_reg as usize];
    let method_val = &vm.stack[offset + method_reg as usize];

    let name = if name_val.is_obj() {
        if let Obj::String(s) = &*name_val.as_obj() {
            s.clone()
        } else {
            return Err("Method name must be a string.".to_string());
        }
    } else {
        return Err("Method name must be a string.".to_string());
    };

    if class_val.is_obj() {
        if let Obj::Class(cls) = &*class_val.as_obj() {
            cls.methods.borrow_mut().insert(name, method_val.clone());
            Ok(())
        } else {
            Err("Method opcode applied to non-class object.".to_string())
        }
    } else {
        Err("Method opcode applied to non-class.".to_string())
    }
}

fn handle_closure(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, const_idx) = decode_inst_imm16(inst);
    let function_val = vm.read_constant(const_idx as usize)?;
    if function_val.is_obj() {
        if let Obj::Function(function) = &*function_val.as_obj() {
            let mut upvalues = Vec::new();
            for req in &function.upvalues {
                if req.is_local {
                    upvalues.push(vm.capture_upvalue(vm.current_frame()?.stack_offset + req.index));
                } else {
                    upvalues.push(vm.current_frame()?.closure.upvalues[req.index].clone());
                }
            }
            let closure = Arc::new(Closure {
                function: function.clone(),
                upvalues,
            });
            let offset = vm.current_frame()?.stack_offset;
            vm.stack[offset + dst as usize] = Value::obj(Arc::new(Obj::Closure(closure)));
            Ok(())
        } else {
            Err("Expected function for closure.".to_string())
        }
    } else {
        Err("Expected function object for closure.".to_string())
    }
}

fn handle_get_upvalue(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, dst, upvalue_idx, _) = decode_inst(inst);
    let upvalue = vm.current_frame()?.closure.upvalues[upvalue_idx as usize].clone();
    let val = match &upvalue.borrow().closed {
        Some(val) => val.clone(),
        None => vm.stack[upvalue.borrow().index].clone(),
    };
    let offset = vm.current_frame()?.stack_offset;
    vm.stack[offset + dst as usize] = val;
    Ok(())
}

fn handle_set_upvalue(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, src, upvalue_idx, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let val = vm.stack[offset + src as usize].clone();
    let upvalue = vm.current_frame()?.closure.upvalues[upvalue_idx as usize].clone();
    if upvalue.borrow().closed.is_some() {
        upvalue.borrow_mut().closed = Some(val);
    } else {
        let index = upvalue.borrow().index;
        vm.stack[index] = val;
    }
    Ok(())
}

fn handle_close_upvalue(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, reg, _, _) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    vm.close_upvalues(offset + reg as usize);
    Ok(())
}

fn handle_setup_handler(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, catch_reg, offset) = decode_inst_imm16(inst);
    let frame_idx = vm.frames.len() - 1;
    let stack_idx = vm.stack.len();
    let catch_ip = vm.current_frame()?.ip + offset as usize;
    vm.handlers.push(crate::vm::ExceptionHandler {
        frame_idx,
        stack_idx,
        catch_ip,
        catch_reg: catch_reg as usize,
    });
    Ok(())
}

fn handle_pop_handler(vm: &mut VM, _inst: u32) -> Result<(), String> {
    vm.handlers.pop();
    Ok(())
}

fn handle_import_module(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, _, const_idx) = decode_inst_imm16(inst);
    let name = vm.read_string(const_idx as usize)?;
    vm.load_module(name, &[])
}

fn handle_import_items(vm: &mut VM, inst: u32) -> Result<(), String> {
    let (_, name_reg, start_reg, count) = decode_inst(inst);
    let offset = vm.current_frame()?.stack_offset;
    let name_val = &vm.stack[offset + name_reg as usize];
    let name = if name_val.is_obj() {
        if let Obj::String(s) = &*name_val.as_obj() {
            s.clone()
        } else {
            return Err("Module name must be a string.".to_string());
        }
    } else {
        return Err("Module name must be a string.".to_string());
    };

    let mut items = Vec::new();
    for i in 0..count {
        let item_val = &vm.stack[offset + start_reg as usize + i as usize];
        if item_val.is_obj() {
            if let Obj::String(s) = &*item_val.as_obj() {
                items.push(s.clone());
            } else {
                return Err("Import item must be a string.".to_string());
            }
        } else {
            return Err("Import item must be a string.".to_string());
        }
    }

    vm.load_module(name, &items)
}
