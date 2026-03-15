pub mod math;
pub mod string;
pub mod array;
pub mod io;
pub mod os;

use crate::vm::VM;

pub fn register_core(vm: &mut VM) {
    // Only register core built-ins globally: emit, type_of, assert, len.
    io::register_core(vm);
    os::register(vm);
}

pub fn load_module(vm: &mut VM, name: &str) -> bool {
    match name {
        "math" => { math::register(vm); true }
        "string" => { string::register(vm); true }
        "array" => { array::register(vm); true }
        "io" => { io::register(vm); true }
        "os" => { os::register(vm); true }
        _ => false,
    }
}
