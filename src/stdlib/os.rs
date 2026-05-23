use std::sync::Arc;
use crate::vm::VM;
use crate::value::Value;
use crate::vm::obj::Obj;

pub fn register(vm: &mut VM) {
    let name = vm.intern("clock");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_clock))));

    let name = vm.intern("exit");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_exit))));

    let name = vm.intern("env");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_env))));

    let name = vm.intern("args");
    vm.globals.insert(name, Value::obj(Arc::new(Obj::NativeFn(native_args))));
}

fn native_clock(_vm: &mut VM, _args: &[Value]) -> Result<Value, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    Ok(Value::float(duration.as_secs_f64()))
}

fn native_exit(_vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    let code = if let Some(v) = args.first() {
        if v.is_int() { v.as_int() as i32 } else { 0 }
    } else {
        0
    };
    std::process::exit(code);
}

fn native_env(vm: &mut VM, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() { return Err("env expects 1 argument (variable name)".to_string()); }
    if args[0].is_obj() {
        if let Obj::String(key) = &*args[0].as_obj() {
            return match std::env::var(&**key) {
                Ok(val) => {
                    let interned = vm.intern(&val);
                    Ok(Value::obj(Arc::new(Obj::String(interned))))
                }
                Err(_) => Ok(Value::null()),
            };
        }
    }
    Err("env: argument must be a string".to_string())
}

fn native_args(vm: &mut VM, _args: &[Value]) -> Result<Value, String> {
    let args: Vec<Value> = std::env::args()
        .map(|a| Value::obj(Arc::new(Obj::String(vm.intern(&a)))))
        .collect();
    Ok(Value::obj(Arc::new(Obj::Array(std::cell::RefCell::new(args)))))
}
