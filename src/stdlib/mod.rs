pub mod math;
pub mod string;
pub mod array;
pub mod io;
pub mod os;

use crate::vm::VM;

pub fn register_all(vm: &mut VM) {
    math::register(vm);
    string::register(vm);
    array::register(vm);
    io::register(vm);
    os::register(vm);
}
