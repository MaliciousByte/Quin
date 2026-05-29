pub mod libcalls;
pub mod codegen;
pub mod types;

#[cfg(feature = "hotaru-jit")]
pub mod hotaru;

use cranelift::prelude::settings;
use cranelift::prelude::Configurable;
use cranelift_jit::{JITBuilder, JITModule};
use crate::value::Function;

pub struct JitEngine {
    pub(crate) ctx: cranelift::codegen::Context,
    pub(crate) module: JITModule,
    pub(crate) fn_counter: usize,
}

impl JitEngine {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        flag_builder.set("opt_level", "speed").unwrap();
        let isa_builder = cranelift_native::builder()
            .unwrap_or_else(|msg| panic!("host machine unsupported: {}", msg));
        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();

        // Register libcall symbols with the JIT
        let mut jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        jit_builder.symbol("quin_array_get", libcalls::quin_array_get as *const u8);
        jit_builder.symbol("quin_array_set", libcalls::quin_array_set as *const u8);
        jit_builder.symbol("quin_call_native_1", libcalls::quin_call_native_1 as *const u8);
        jit_builder.symbol("quin_get_global", libcalls::quin_get_global as *const u8);
        jit_builder.symbol("quin_call_generic", libcalls::quin_call_generic as *const u8);

        let module = JITModule::new(jit_builder);
        Self { ctx: cranelift::codegen::Context::new(), module, fn_counter: 0 }
    }

    pub fn compile(&mut self, function: &Function) -> *const u8 {
        codegen::compile_function(self, function)
    }
}
