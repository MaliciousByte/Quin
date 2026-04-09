use std::sync::Arc;
use crate::value::{Value, Closure};
use crate::obj::Obj;
use std::path::{Path, PathBuf};
use std::fs;
use super::VM;

pub enum ModuleState {
    Loading,
    Loaded,
}

impl VM {
    pub fn set_script_dir(&mut self, path: &str) {
        if let Some(parent) = Path::new(path).parent() {
            self.script_dir = Some(parent.to_path_buf());
        }
    }

    pub fn load_module(&mut self, name: Arc<str>, _selective_items: &[Arc<str>]) -> Result<(), String> {
        if let Some(state) = self.module_states.get(&name) {
            match state {
                ModuleState::Loaded => return Ok(()),
                ModuleState::Loading => return Err(format!("Circular import detected: module '{}' is already being loaded", name)),
            }
        }

        self.module_states.insert(name.clone(), ModuleState::Loading);

        let is_stdlib = crate::stdlib::load_module(self, &name);
        
        if !is_stdlib {
            // File import
            let ext = if name.ends_with(".qn") { "" } else { ".qn" };
            let filename = format!("{}{}", name, ext);
            
            let path = if let Some(dir) = &self.script_dir {
                dir.join(&filename)
            } else {
                PathBuf::from(&filename)
            };

            let source = fs::read_to_string(&path)
                .map_err(|e| format!("Could not load module '{}' at {}: {}", name, path.display(), e))?;

            let mut lexer = crate::lexer::Lexer::new(&source);
            let tokens = lexer.scan_tokens().map_err(|e| format!("Lexer error in module '{}': {}", name, e))?;

            let mut parser = crate::parser::Parser::new(tokens);
            let ast = parser.parse().map_err(|e| format!("Parser error in module '{}': {}", name, e))?;

            let path_str = path.to_string_lossy().to_string();
            let compiler = crate::compiler::Compiler::new(&path_str, false, false, None);
            let function = compiler.compile(&ast).map_err(|e| format!("Compiler error in module '{}': {}", name, e))?;

            let old_frames_len = self.frames.len();
            let old_stack_len = self.stack.len();

            let closure = Arc::new(Closure {
                function: Arc::new(function),
                upvalues: Vec::new(),
            });
            
            // Push closure for the call
            self.stack.push(Value::obj(Arc::new(Obj::Closure(closure.clone()))));
            self.call_closure(closure, 0)?;
            
            // Run the module script
            while self.frames.len() > old_frames_len {
                let op = self.read_instruction()?;
                self.execute_op(op)?;
            }
            
            // Clean up stack
            self.stack.truncate(old_stack_len);
        }

        self.module_states.insert(name, ModuleState::Loaded);
        Ok(())
    }
}
