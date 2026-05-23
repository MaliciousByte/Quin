use std::sync::Arc;
use crate::vm::VM;
use crate::value::Value;
use crate::vm::obj::Obj;

pub fn register(vm: &mut VM) {
    let name = vm.intern("upper");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_upper))));

    let name = vm.intern("lower");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_lower))));

    let name = vm.intern("trim");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_trim))));

    let name = vm.intern("contains");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_contains))));

    let name = vm.intern("replace");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_replace))));

    let name = vm.intern("split");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_split))));

    let name = vm.intern("starts_with");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_starts_with))));

    let name = vm.intern("ends_with");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_ends_with))));

    let name = vm.intern("to_str");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_to_str))));

    let name = vm.intern("to_int");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_to_int))));

    let name = vm.intern("to_float");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_to_float))));
}

fn extract_string(v: &Value) -> Result<String, String> {
    if v.is_obj() {
        if let Obj::String(s) = &*v.as_obj() {
            return Ok(s.to_string());
        }
    }
    Err("Expected a string".to_string())
}

pub fn native_len(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("len expects 1 argument")?;
    if v.is_obj() {
        match &*v.as_obj() {
            Obj::String(s) => return Ok(Value::int(s.len() as i64)),
            Obj::Array(arr) => return Ok(Value::int(arr.borrow().len() as i64)),
            Obj::Dict(d) => return Ok(Value::int(d.borrow().len() as i64)),
            Obj::Set(s) => return Ok(Value::int(s.borrow().len() as i64)),
            Obj::Tuple(t) => return Ok(Value::int(t.len() as i64)),
            _ => {}
        }
    }
    Err("len: unsupported type".to_string())
}

fn native_upper(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let s = extract_string(args.first().ok_or("upper expects 1 argument")?)?;
    let interned = vm.intern(&s.to_uppercase());
    Ok(Value::obj(Arc::new(Obj::String(interned))))
}

fn native_lower(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let s = extract_string(args.first().ok_or("lower expects 1 argument")?)?;
    let interned = vm.intern(&s.to_lowercase());
    Ok(Value::obj(Arc::new(Obj::String(interned))))
}

fn native_trim(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let s = extract_string(args.first().ok_or("trim expects 1 argument")?)?;
    let interned = vm.intern(s.trim());
    Ok(Value::obj(Arc::new(Obj::String(interned))))
}

fn native_contains(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("contains expects 2 arguments".to_string()); }
    let haystack = extract_string(&args[0])?;
    let needle = extract_string(&args[1])?;
    Ok(Value::bool(haystack.contains(&needle)))
}

fn native_replace(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 3 { return Err("replace expects 3 arguments (str, from, to)".to_string()); }
    let s = extract_string(&args[0])?;
    let from = extract_string(&args[1])?;
    let to = extract_string(&args[2])?;
    let interned = vm.intern(&s.replace(&from, &to));
    Ok(Value::obj(Arc::new(Obj::String(interned))))
}

fn native_split(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("split expects 2 arguments (str, delimiter)".to_string()); }
    let s = extract_string(&args[0])?;
    let delim = extract_string(&args[1])?;
    let parts: Vec<Value> = s.split(&delim)
        .map(|p| Value::obj(Arc::new(Obj::String(vm.intern(p)))))
        .collect();
    Ok(Value::obj(Arc::new(Obj::Array(std::cell::RefCell::new(parts)))))
}

fn native_starts_with(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("starts_with expects 2 arguments".to_string()); }
    let s = extract_string(&args[0])?;
    let prefix = extract_string(&args[1])?;
    Ok(Value::bool(s.starts_with(&prefix)))
}

fn native_ends_with(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("ends_with expects 2 arguments".to_string()); }
    let s = extract_string(&args[0])?;
    let suffix = extract_string(&args[1])?;
    Ok(Value::bool(s.ends_with(&suffix)))
}

fn native_to_str(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("to_str expects 1 argument")?;
    let interned = vm.intern(&format!("{}", v));
    Ok(Value::obj(Arc::new(Obj::String(interned))))
}

fn native_to_int(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("to_int expects 1 argument")?;
    if v.is_int() { return Ok(v.clone()); }
    if v.is_float() { return Ok(Value::int(v.as_float() as i64)); }
    if v.is_obj() {
        if let Obj::String(s) = &*v.as_obj() {
            return s.parse::<i64>().map(Value::int)
                .map_err(|_| format!("Cannot parse '{}' as int", s));
        }
    }
    Err("to_int: unsupported type".to_string())
}

fn native_to_float(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("to_float expects 1 argument")?;
    if v.is_float() { return Ok(v.clone()); }
    if v.is_int() { return Ok(Value::float(v.as_int() as f64)); }
    if v.is_obj() {
        if let Obj::String(s) = &*v.as_obj() {
            return s.parse::<f64>().map(Value::float)
                .map_err(|_| format!("Cannot parse '{}' as float", s));
        }
    }
    Err("to_float: unsupported type".to_string())
}
