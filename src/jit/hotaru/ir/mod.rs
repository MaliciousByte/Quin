// IR sub-module for Hotaru JIT
pub mod hcir;
pub mod lift;

pub use hcir::{HcirOp, LiftResult};
pub use lift::lift_function;
