use crate::value::Value;
use crate::vm::obj::Obj;

// ─────────────────────────────────────────────────────────────────────────────
// LIBCALL HELPERS — extern "C" functions callable from JIT-compiled code
// ─────────────────────────────────────────────────────────────────────────────

/// Array get: extract element at index. arr_bits and idx_bits are NaN-boxed Values.
/// Returns the raw u64 bits of the element Value (no refcount change — parent
/// array keeps the element alive for the duration of the JIT frame).
#[no_mangle]
pub extern "C" fn quin_array_get(arr_bits: i64, idx_bits: i64) -> i64 {
    let arr_val = Value(arr_bits as u64);
    let idx_val = Value(idx_bits as u64);
    let result = if arr_val.is_obj() {
        let obj = arr_val.as_obj();
        match &*obj {
            Obj::Array(arr) => {
                if idx_val.is_int() {
                    let i = idx_val.as_int();
                    let elements = arr.borrow();
                    if i >= 0 && (i as usize) < elements.len() {
                        let bits = elements[i as usize].0;
                        let v = &elements[i as usize];
                        v.mark(); // increment refcount for the returned reference
                        bits as i64
                    } else {
                        Value::null().0 as i64
                    }
                } else {
                    Value::null().0 as i64
                }
            }
            _ => Value::null().0 as i64,
        }
    } else {
        Value::null().0 as i64
    };
    std::mem::forget(arr_val);
    std::mem::forget(idx_val);
    result
}

/// Array set: set element at index. Returns val_bits.
/// arr_bits, idx_bits, val_bits are all NaN-boxed Values.
#[no_mangle]
pub extern "C" fn quin_array_set(arr_bits: i64, idx_bits: i64, val_bits: i64) -> i64 {
    let arr_val = Value(arr_bits as u64);
    let idx_val = Value(idx_bits as u64);
    let new_val = Value(val_bits as u64);
    if arr_val.is_obj() {
        let obj = arr_val.as_obj();
        match &*obj {
            Obj::Array(arr) => {
                if idx_val.is_int() {
                    let i = idx_val.as_int();
                    let mut elements = arr.borrow_mut();
                    if i >= 0 && (i as usize) < elements.len() {
                        elements[i as usize] = new_val.clone();
                    }
                }
            }
            _ => {}
        }
    }
    let result = val_bits;
    std::mem::forget(arr_val);
    std::mem::forget(idx_val);
    std::mem::forget(new_val);
    result
}

/// Call a 1-arg native function. vm_ptr is *mut VM, fn_bits is the NaN-boxed
/// closure/NativeFn value, arg_bits is the single argument.
/// Returns NaN-boxed result bits.
#[no_mangle]
pub extern "C" fn quin_call_native_1(vm_ptr: *mut crate::vm::VM, fn_bits: i64, arg_bits: i64) -> i64 {
    let fn_val = Value(fn_bits as u64);
    let arg_val = Value(arg_bits as u64);
    let result = if fn_val.is_obj() {
        let obj = fn_val.as_obj();
        match &*obj {
            Obj::NativeFn(native) => {
                let vm = unsafe { &mut *vm_ptr };
                match native(vm, &[arg_val.clone()]) {
                    Ok(v) => {
                        let bits = v.0 as i64;
                        std::mem::forget(v);
                        bits
                    }
                    Err(_) => {
                        let v = Value::null();
                        let bits = v.0 as i64;
                        std::mem::forget(v);
                        bits
                    }
                }
            }
            Obj::Closure(closure) => {
                let vm = unsafe { &mut *vm_ptr };
                let closure = closure.clone();
                let caller_offset = if let Ok(f) = vm.current_frame() { f.stack_offset } else { 0 };
                let callee_reg = (vm.stack.len() - caller_offset) as u8;
                vm.push(fn_val.clone());
                vm.push(arg_val.clone());
                let starting_frames = vm.frames.len();
                match vm.call_closure(closure, 1, callee_reg, None) {
                    Ok(_) => {
                        if vm.frames.len() > starting_frames {
                            if let Err(_) = vm.run() {
                                return Value::null().0 as i64;
                            }
                        }
                        let v = vm.pop().unwrap_or(Value::null());
                        let bits = v.0 as i64;
                        std::mem::forget(v);
                        bits
                    }
                    Err(_) => {
                        let v = Value::null();
                        let bits = v.0 as i64;
                        std::mem::forget(v);
                        bits
                    }
                }
            }
            _ => {
                let v = Value::null();
                let bits = v.0 as i64;
                std::mem::forget(v);
                bits
            }
        }
    } else {
        let v = Value::null();
        let bits = v.0 as i64;
        std::mem::forget(v);
        bits
    };
    std::mem::forget(fn_val);
    std::mem::forget(arg_val);
    result
}

/// Get a global variable by name. vm_ptr is *mut VM, const_ptr is a pointer
/// to the constants array, const_idx is the index of the string constant.
/// Returns NaN-boxed bits of the global value.
#[no_mangle]
pub extern "C" fn quin_get_global(vm_ptr: *mut crate::vm::VM, const_ptr: *const Value, const_idx: i64) -> i64 {
    let vm = unsafe { &mut *vm_ptr };
    let constants = unsafe { &*std::ptr::slice_from_raw_parts(const_ptr, (const_idx as usize) + 1) };
    let name_val = &constants[const_idx as usize];
    if name_val.is_obj() {
        let obj = name_val.as_obj();
        match &*obj {
            Obj::String(s) => {
                if let Some(val) = vm.globals.get(s) {
                    let bits = val.0 as i64;
                    val.mark(); // increment refcount for the returned reference
                    bits
                } else {
                    Value::null().0 as i64
                }
            }
            _ => Value::null().0 as i64,
        }
    } else {
        Value::null().0 as i64
    }
}

/// Generic call: call any Quin value from JIT-compiled code.
/// vm_ptr is *mut VM, callee_bits is NaN-boxed callee value,
/// args_ptr points to an array of NaN-boxed arg values (as i64),
/// arg_count is the number of arguments.
/// Returns NaN-boxed result bits.
#[no_mangle]
pub extern "C" fn quin_call_generic(
    vm_ptr: *mut crate::vm::VM,
    callee_bits: i64,
    args_ptr: *const i64,
    arg_count: i64,
) -> i64 {
    let vm = unsafe { &mut *vm_ptr };
    let callee = Value(callee_bits as u64);

    // Build args Vec from the pointer
    let mut args = Vec::with_capacity(arg_count as usize);
    for i in 0..arg_count as usize {
        let bits = unsafe { *args_ptr.add(i) };
        let v = Value(bits as u64);
        v.mark(); // increment refcount for the arg copy
        args.push(v);
    }

    // Use call_value_native which handles all callee types
    let caller_offset = if let Ok(f) = vm.current_frame() { f.stack_offset } else { 0 };
    let callee_reg = (vm.stack.len() - caller_offset) as u8;
    vm.push(callee.clone());
    for arg in &args {
        vm.push(arg.clone());
    }

    let starting_frames = vm.frames.len();
    match vm.call_value(callee_reg, arg_count as u8, None) {
        Ok(_) => {
            if vm.frames.len() > starting_frames {
                if let Err(_) = vm.run() {
                    std::mem::forget(callee);
                    for a in args { std::mem::forget(a); }
                    return Value::null().0 as i64;
                }
            }
            let v = vm.pop().unwrap_or(Value::null());
            let bits = v.0 as i64;
            std::mem::forget(v);
            std::mem::forget(callee);
            for a in args { std::mem::forget(a); }
            bits
        }
        Err(_) => {
            std::mem::forget(callee);
            for a in args { std::mem::forget(a); }
            Value::null().0 as i64
        }
    }
}
