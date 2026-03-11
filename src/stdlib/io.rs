use std::rc::Rc;
use crate::vm::VM;
use crate::value::Value;
use crate::obj::Obj;

pub fn register(vm: &mut VM) {
    // emit is already registered in VM::new(), but we re-register here for consistency
    let name = vm.intern("emit");
    vm.globals.insert(name, Value::obj(Rc::new(Obj::NativeFn(native_emit))));

    let name = vm.intern("input");
    vm.globals.insert(name, Value::obj(Rc::new(Obj::NativeFn(native_input))));

    let name = vm.intern("read_file");
    vm.globals.insert(name, Value::obj(Rc::new(Obj::NativeFn(native_read_file))));

    let name = vm.intern("write_file");
    vm.globals.insert(name, Value::obj(Rc::new(Obj::NativeFn(native_write_file))));

    let name = vm.intern("type_of");
    vm.globals.insert(name, Value::obj(Rc::new(Obj::NativeFn(native_type_of))));
}

fn native_emit(args: &[Value]) -> Result<Value, String> {
    if let Some(val) = args.first() {
        println!("{}", val);
    } else {
        println!();
    }
    Ok(Value::null())
}

fn native_input(args: &[Value]) -> Result<Value, String> {
    use std::io::{self, Write};
    if let Some(prompt) = args.first() {
        print!("{}", prompt);
        io::stdout().flush().map_err(|e| e.to_string())?;
    }
    let mut line = String::new();
    io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
    let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
    Ok(Value::obj(Rc::new(Obj::String(Rc::from(trimmed)))))
}

fn native_read_file(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() { return Err("read_file expects 1 argument (path)".to_string()); }
    if args[0].is_obj() {
        if let Obj::String(path) = &*args[0].as_obj() {
            let content = std::fs::read_to_string(&**path)
                .map_err(|e| format!("read_file error: {}", e))?;
            return Ok(Value::obj(Rc::new(Obj::String(Rc::from(content.as_str())))));
        }
    }
    Err("read_file: argument must be a string path".to_string())
}

fn native_write_file(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 { return Err("write_file expects 2 arguments (path, content)".to_string()); }
    let path = if args[0].is_obj() {
        if let Obj::String(s) = &*args[0].as_obj() { s.to_string() }
        else { return Err("write_file: path must be a string".to_string()); }
    } else {
        return Err("write_file: path must be a string".to_string());
    };
    let content = if args[1].is_obj() {
        if let Obj::String(s) = &*args[1].as_obj() { s.to_string() }
        else { format!("{}", args[1]) }
    } else {
        format!("{}", args[1])
    };
    std::fs::write(&path, &content).map_err(|e| format!("write_file error: {}", e))?;
    Ok(Value::null())
}

fn native_type_of(args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("type_of expects 1 argument")?;
    let type_name = if v.is_int() { "int" }
    else if v.is_float() { "float" }
    else if v.is_bool() { "bool" }
    else if v.is_null() { "void" }
    else if v.is_obj() { v.as_obj().type_name() }
    else { "unknown" };
    Ok(Value::obj(Rc::new(Obj::String(Rc::from(type_name)))))
}
