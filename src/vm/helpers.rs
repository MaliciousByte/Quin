use std::sync::Arc;
use crate::value::Value;
use crate::vm::obj::Obj;
use super::{VM, CallFrame};

impl VM {
    #[inline(always)]
    pub(crate) fn read_instruction_u32(&mut self) -> Result<u32, String> {
        let frame = self.current_frame_mut()?;
        if frame.ip >= frame.closure.function.chunk.code.len() {
            return Err("Execution reached end of chunk without returning.".to_string());
        }
        let op = frame.closure.function.chunk.code[frame.ip];
        frame.ip += 1;
        Ok(op)
    }

    #[inline(always)]
    pub(crate) fn read_constant(&self, idx: usize) -> Result<Value, String> {
        let frame = self.current_frame()?;
        if idx >= frame.closure.function.chunk.constants.len() {
            return Err(format!("Constant index {} out of bounds.", idx));
        }
        Ok(frame.closure.function.chunk.constants[idx].clone())
    }

    pub(crate) fn read_string(&self, idx: usize) -> Result<Arc<str>, String> {
        let val = self.read_constant(idx)?;
        if val.is_obj() {
            if let Obj::String(s) = &*val.as_obj() {
                return Ok(s.clone());
            }
        }
        Err("Expected string constant.".to_string())
    }

    #[inline(always)]
    pub fn push(&mut self, value: Value) {
        if self.stack.len() >= super::STACK_MAX {
            panic!("Stack overflow");
        }
        self.stack.push(value);
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Result<Value, String> {
        self.stack.pop().ok_or_else(|| "Stack underflow.".to_string())
    }

    #[inline(always)]
    pub(crate) fn peek(&self, distance: usize) -> Result<&Value, String> {
        if distance >= self.stack.len() {
            Err("Stack underflow on peek.".to_string())
        } else {
            Ok(&self.stack[self.stack.len() - 1 - distance])
        }
    }

    pub fn is_falsey(&self, value: &Value) -> bool {
        if value.is_null() { return true; }
        if value.is_bool() { return !value.as_bool(); }
        if value.is_int() { return value.as_int() == 0; }
        if value.is_obj() {
            match &*value.as_obj() {
                Obj::Array(a) => a.borrow().is_empty(),
                Obj::Dict(d) => d.borrow().is_empty(),
                Obj::Set(s) => s.borrow().is_empty(),
                Obj::Object(inst) => inst.borrow().fields.borrow().is_empty(),
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn intern(&mut self, s: &str) -> Arc<str> {
        self.interner.intern(s)
    }

    #[inline(always)]
    pub(crate) fn current_frame(&self) -> Result<&CallFrame, String> {
        self.frames.last().ok_or_else(|| "No call frame.".to_string())
    }

    #[inline(always)]
    pub(crate) fn current_frame_mut(&mut self) -> Result<&mut CallFrame, String> {
        self.frames.last_mut().ok_or_else(|| "No call frame.".to_string())
    }

    pub(crate) fn handle_exception(&mut self, error: Value) -> Result<(), String> {
        if let Some(handler) = self.handlers.pop() {
            // Unwind frames
            self.frames.truncate(handler.frame_idx + 1);
            // Unwind stack
            self.stack.truncate(handler.stack_idx);
            
            // Set the exception value in the catch register
            let offset = self.current_frame()?.stack_offset;
            self.stack[offset + handler.catch_reg] = error;

            // Jump to catch
            self.current_frame_mut()?.ip = handler.catch_ip;
            Ok(())
        } else {
            Err(format!("Uncaught error: {}", error))
        }
    }
}
