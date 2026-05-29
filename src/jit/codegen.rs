use crate::value::Function;
use super::JitEngine;

pub(crate) fn compile_function(_engine: &mut JitEngine, function: &Function) -> *const u8 {
    #[cfg(feature = "hotaru-jit")]
    {
        println!("[DEBUG] Compiling function {:?}", function.name);
        let lift_result = crate::jit::hotaru::ir::lift::lift_function(function);
        let code_ptr = crate::jit::hotaru::backend::emit::compile_hotaru(function, &lift_result);
        println!("[DEBUG] Finished compiling function {:?}, ptr = {:?}", function.name, code_ptr);
        return code_ptr;
    }

    #[cfg(not(feature = "hotaru-jit"))]
    {
        let _ = function;
        std::ptr::null()
    }
}
