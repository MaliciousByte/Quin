use std::sync::Arc;
use crate::vm::VM;
use crate::value::Value;
use crate::obj::Obj;

pub fn register(vm: &mut VM) {
    // sqrt - already exists but we re-register for consistency
    let name = vm.intern("sqrt");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_sqrt))));

    let name = vm.intern("pow");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_pow))));

    let name = vm.intern("abs");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_abs))));

    let name = vm.intern("floor");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_floor))));

    let name = vm.intern("ceil");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_ceil))));

    let name = vm.intern("round");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_round))));

    let name = vm.intern("min");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_min))));

    let name = vm.intern("max");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_max))));

    let name = vm.intern("PI");
    vm.globals.insert(name, Value::float(std::f64::consts::PI));

    let name = vm.intern("E");
    vm.globals.insert(name, Value::float(std::f64::consts::E));
}

fn to_f64(v: &Value) -> Result<f64, String> {
    if v.is_float() { Ok(v.as_float()) }
    else if v.is_int() { Ok(v.as_int() as f64) }
    else { Err("Expected a number".to_string()) }
}

fn native_sqrt(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let v = to_f64(args.first().ok_or("sqrt expects 1 argument")?)?;
    Ok(Value::float(v.sqrt()))
}

fn native_pow(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("pow expects 2 arguments".to_string()); }
    let base = to_f64(&args[0])?;
    let exp = to_f64(&args[1])?;
    Ok(Value::float(base.powf(exp)))
}

fn native_abs(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("abs expects 1 argument")?;
    if v.is_int() { Ok(Value::int(v.as_int().abs())) }
    else if v.is_float() { Ok(Value::float(v.as_float().abs())) }
    else { Err("abs expects a number".to_string()) }
}

fn native_floor(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let v = to_f64(args.first().ok_or("floor expects 1 argument")?)?;
    Ok(Value::int(v.floor() as i64))
}

fn native_ceil(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let v = to_f64(args.first().ok_or("ceil expects 1 argument")?)?;
    Ok(Value::int(v.ceil() as i64))
}

fn native_round(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let v = to_f64(args.first().ok_or("round expects 1 argument")?)?;
    Ok(Value::int(v.round() as i64))
}

fn native_min(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("min expects 2 arguments".to_string()); }
    let a = to_f64(&args[0])?;
    let b = to_f64(&args[1])?;
    if a < b {
        Ok(args[0].clone())
    } else {
        Ok(args[1].clone())
    }
}

fn native_max(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("max expects 2 arguments".to_string()); }
    let a = to_f64(&args[0])?;
    let b = to_f64(&args[1])?;
    if a > b {
        Ok(args[0].clone())
    } else {
        Ok(args[1].clone())
    }
}
