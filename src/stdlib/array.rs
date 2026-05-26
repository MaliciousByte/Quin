use std::sync::Arc;
use std::cell::RefCell;
use crate::vm::VM;
use crate::value::Value;
use crate::vm::obj::Obj;

pub fn register(vm: &mut VM) {
    let name = vm.intern("push");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_push))));

    let name = vm.intern("pop");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_pop))));

    let name = vm.intern("slice");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_slice))));

    let name = vm.intern("reverse");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_reverse))));

    let name = vm.intern("sort");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_sort))));

    let name = vm.intern("range");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_range))));

    let name = vm.intern("join");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_join))));

    let name = vm.intern("map");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_map))));

    let name = vm.intern("filter");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_filter))));
}

fn native_push(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("push expects 2 arguments (array, value)".to_string()); }
    if args[0].is_obj() {
        if let Obj::Array(arr) = &*args[0].as_obj() {
            arr.borrow_mut().push(args[1].clone());
            return Ok(Value::null());
        }
    }
    Err("push: first argument must be an array".to_string())
}

fn native_pop(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() { return Err("pop expects 1 argument (array)".to_string()); }
    if args[0].is_obj() {
        if let Obj::Array(arr) = &*args[0].as_obj() {
            return arr.borrow_mut().pop().ok_or("pop: array is empty".to_string());
        }
    }
    Err("pop: argument must be an array".to_string())
}

fn native_slice(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 || args.len() > 3 { return Err("slice expects 2-3 arguments (array, start, end?)".to_string()); }
    if args[0].is_obj() {
        if let Obj::Array(arr) = &*args[0].as_obj() {
            let elements = arr.borrow();
            let start = if args[1].is_int() { args[1].as_int().max(0) as usize } else { return Err("slice: start must be int".to_string()); };
            let end = if args.len() == 3 {
                if args[2].is_int() { args[2].as_int().min(elements.len() as i64) as usize }
                else { return Err("slice: end must be int".to_string()); }
            } else {
                elements.len()
            };
            let sliced: Vec<Value> = elements[start..end.min(elements.len())].to_vec();
            return Ok(Value::obj(Arc::new(Obj::Array(RefCell::new(sliced)))));
        }
    }
    Err("slice: first argument must be an array".to_string())
}

fn native_reverse(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() { return Err("reverse expects 1 argument (array)".to_string()); }
    if args[0].is_obj() {
        if let Obj::Array(arr) = &*args[0].as_obj() {
            arr.borrow_mut().reverse();
            return Ok(Value::null());
        }
    }
    Err("reverse: argument must be an array".to_string())
}

fn native_sort(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() { return Err("sort expects 1 argument (array)".to_string()); }
    if args[0].is_obj() {
        if let Obj::Array(arr) = &*args[0].as_obj() {
            let mut elements = arr.borrow_mut();
            elements.sort_by(|a, b| {
                if a.is_int() && b.is_int() {
                    a.as_int().cmp(&b.as_int())
                } else if a.is_float() && b.is_float() {
                    a.as_float().partial_cmp(&b.as_float()).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    std::cmp::Ordering::Equal
                }
            });
            return Ok(Value::null());
        }
    }
    Err("sort: argument must be an array".to_string())
}

fn native_range(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() || args.len() > 3 { return Err("range expects 1-3 arguments (end) or (start, end) or (start, end, step)".to_string()); }
    let (start, end, step) = match args.len() {
        1 => {
            if !args[0].is_int() { return Err("range: arguments must be integers".to_string()); }
            (0i64, args[0].as_int(), 1i64)
        }
        2 => {
            if !args[0].is_int() || !args[1].is_int() { return Err("range: arguments must be integers".to_string()); }
            (args[0].as_int(), args[1].as_int(), 1i64)
        }
        3 => {
            if !args[0].is_int() || !args[1].is_int() || !args[2].is_int() {
                return Err("range: arguments must be integers".to_string());
            }
            (args[0].as_int(), args[1].as_int(), args[2].as_int())
        }
        _ => unreachable!()
    };

    if step == 0 { return Err("range: step cannot be zero".to_string()); }

    let mut result = Vec::new();
    let mut i = start;
    if step > 0 {
        while i < end {
            result.push(Value::int(i));
            i += step;
        }
    } else {
        while i > end {
            result.push(Value::int(i));
            i += step;
        }
    }
    Ok(Value::obj(Arc::new(Obj::Array(RefCell::new(result)))))
}

fn native_join(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("join expects 2 arguments (array, separator)".to_string()); }
    if args[0].is_obj() {
        if let Obj::Array(arr) = &*args[0].as_obj() {
            let sep = if args[1].is_obj() {
                if let Obj::String(s) = &*args[1].as_obj() {
                    s.to_string()
                } else {
                    return Err("join: separator must be a string".to_string());
                }
            } else {
                return Err("join: separator must be a string".to_string());
            };
            let elements = arr.borrow();
            let parts: Vec<String> = elements.iter().map(|v| format!("{}", v)).collect();
            let interned = vm.intern(&parts.join(&sep));
            return Ok(Value::obj(Arc::new(Obj::String(interned))));
        }
    }
    Err("join: first argument must be an array".to_string())
}

fn native_map(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("map expects 2 arguments (array, callback)".to_string()); }
    let (elements, callback) = if args[0].is_obj() {
        if let Obj::Array(arr) = &*args[0].as_obj() {
            (arr.borrow().clone(), args[1].clone())
        } else {
            return Err("map: first argument must be an array".to_string());
        }
    } else {
        return Err("map: first argument must be an array".to_string());
    };

    let mut result_elements = Vec::with_capacity(elements.len());
    for element in elements {
        vm.call_value_native(callback.clone(), &[element])?;
        vm.run()?;
        result_elements.push(vm.pop()?);
    }

    Ok(Value::obj(Arc::new(Obj::Array(RefCell::new(result_elements)))))
}

fn native_filter(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("filter expects 2 arguments (array, callback)".to_string()); }
    let (elements, callback) = if args[0].is_obj() {
        if let Obj::Array(arr) = &*args[0].as_obj() {
            (arr.borrow().clone(), args[1].clone())
        } else {
            return Err("filter: first argument must be an array".to_string());
        }
    } else {
        return Err("filter: first argument must be an array".to_string());
    };

    let mut result_elements = Vec::new();
    for element in elements {
        vm.call_value_native(callback.clone(), &[element.clone()])?;
        vm.run()?;
        let res = vm.pop()?;
        if !vm.is_falsey(&res) {
            result_elements.push(element);
        }
    }

    Ok(Value::obj(Arc::new(Obj::Array(RefCell::new(result_elements)))))
}

