use std::collections::HashMap;
use crate::ast::{Expr, Stmt, Literal};
use crate::chunk::{Chunk, OpCode};
use crate::token::{Token, TokenType};
use crate::value::{Value, Function};
use crate::obj::Obj;
use std::sync::Arc;

pub struct Local {
    pub name: String,
    pub depth: usize,
    pub is_captured: bool,
}

#[derive(Clone, Copy)]
pub struct UpvalueMetadata {
    pub index: usize,
    pub is_local: bool,
}

pub struct Compiler {
    pub function: Function,
    pub parent: Option<*mut Compiler>,
    pub locals: Vec<Local>,
    pub upvalues: Vec<UpvalueMetadata>,
    pub scope_depth: usize,
    pub globals: HashMap<String, usize>,
    pub emitting_method: bool,
}

impl Compiler {
    pub fn new(name: &str, is_async: bool, is_method: bool, parent: Option<*mut Compiler>) -> Self {
        let mut compiler = Compiler {
            function: Function {
                name: Arc::from(name),
                arity: 0,
                max_locals: 0,
                is_async,
                chunk: Chunk::new(),
                upvalues: Vec::new(),
                call_count: std::sync::atomic::AtomicU32::new(0),
                is_hot: std::sync::atomic::AtomicBool::new(false),
                native_ptr: std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()),
            },
            parent,
            locals: Vec::new(),
            upvalues: Vec::new(),
            scope_depth: 0,
            globals: HashMap::new(),
            emitting_method: false,
        };
        // slot 0 for local call frame
        let slot0_name = if is_method { "self".to_string() } else { "".to_string() };
        compiler.locals.push(Local { name: slot0_name, depth: 0, is_captured: false });
        compiler.function.max_locals = 1;
        compiler
    }

    pub fn compile(mut self, stmts: &[Stmt]) -> Result<Function, String> {
        for stmt in stmts {
            self.compile_stmt(stmt)?;
        }
        self.emit(OpCode::Return, 0); // slot 0 return is default
        Ok(self.function)
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Expression(expr) => {
                self.compile_expr(expr)?;
                self.emit(OpCode::Pop, 0); // pop result off stack
            }

            Stmt::Let { pattern, initializer, .. } => {
                if let Some(init) = initializer {
                    self.compile_expr(init)?;
                } else {
                    self.emit(OpCode::Null, 0);
                }

                match pattern {
                    crate::ast::Pattern::Identifier(name) => {
                        if self.scope_depth > 0 {
                            self.locals.push(Local {
                                name: name.lexeme.clone(),
                                depth: self.scope_depth,
                                is_captured: false,
                            });
                            if self.locals.len() > self.function.max_locals {
                                self.function.max_locals = self.locals.len();
                            }
                        } else {
                            let idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                            self.emit(OpCode::DefineGlobal(idx), name.line);
                        }
                    }
                    _ => return Err("Destructuring in 'let' not yet implemented in compiler.".to_string()),
                }
            }
            Stmt::Block(stmts) => {
                self.begin_scope();
                for s in stmts {
                    self.compile_stmt(s)?;
                }
                self.end_scope();
            }
            Stmt::If { condition, then_branch, elif_branches, else_branch } => {
                self.compile_expr(condition)?;
                let then_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                self.emit(OpCode::Pop, 0);
                self.compile_stmt(then_branch)?;
                let mut end_jumps = vec![self.emit_jump(OpCode::Jump(0))];
                
                self.patch_jump(then_jump);
                self.emit(OpCode::Pop, 0);
                
                for (elif_cond, elif_body) in elif_branches {
                    self.compile_expr(elif_cond)?;
                    let elif_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                    self.emit(OpCode::Pop, 0);
                    self.compile_stmt(elif_body)?;
                    end_jumps.push(self.emit_jump(OpCode::Jump(0)));
                    self.patch_jump(elif_jump);
                    self.emit(OpCode::Pop, 0);
                }
                
                if let Some(eb) = else_branch {
                    self.compile_stmt(eb)?;
                }
                
                for jump in end_jumps {
                    self.patch_jump(jump);
                }
            }
            Stmt::While { condition, body } => {
                let loop_start = self.current_chunk().code.len();
                self.compile_expr(condition)?;
                let exit_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                self.emit(OpCode::Pop, 0);
                self.compile_stmt(body)?;
                self.emit_loop(loop_start);
                self.patch_jump(exit_jump);
                self.emit(OpCode::Pop, 0);
            }
            Stmt::For { item: _, iterable: _, body: _ } => {
                // For simplicity, implement a naive while loop over array
                // e.g. `for i in arr` => `let mut idx = 0; while idx < arr.len { let i = arr[idx]; body; idx+=1 }`
                // Emitting basic logic (not robust iterator)
                return Err("For loops require iterator protocol or array length logic via stdlib, simplifying for VM v1.".to_string());
            }
            Stmt::Function { name, params, variadic: _, is_async, is_static, is_abstract, visibility, body, .. } => {
                let is_method = self.emitting_method; // Use emitting_method flag from outer compiler
                let mut compiler = Compiler::new(&name.lexeme, *is_async, is_method, Some(self as *mut Compiler));
                compiler.function.arity = params.len();
                // TODO: handle variadic, default values, visibility, abstract, and static in VM/Compiler logic
                let _ = is_static;
                let _ = visibility;
                let _ = is_abstract;
                compiler.begin_scope();
                for (p, _, _) in params {
                    compiler.locals.push(Local { name: p.lexeme.clone(), depth: compiler.scope_depth, is_captured: false });
                }
                compiler.function.max_locals = compiler.locals.len();
                for s in body {
                    compiler.compile_stmt(s)?;
                }
                if name.lexeme == "init" || name.lexeme == "constructor" {
                    compiler.emit(OpCode::GetLocal(0), 0);
                } else {
                    compiler.emit(OpCode::Null, 0);
                }
                compiler.emit(OpCode::Return, 0);

                let fun = compiler.function;
                let idx = self.add_constant(Value::obj(Arc::new(Obj::Function(Arc::new(fun)))));
                
                if self.emitting_method {
                    self.emit(OpCode::Closure(idx), name.line);
                } else if self.scope_depth > 0 {
                    self.emit(OpCode::Closure(idx), name.line);
                    self.locals.push(Local { name: name.lexeme.clone(), depth: self.scope_depth, is_captured: false });
                } else {
                    let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                    self.emit(OpCode::Closure(idx), name.line);
                    self.emit(OpCode::DefineGlobal(name_idx), name.line);
                }
            }
            Stmt::Return { keyword, value } => {
                if let Some(v) = value {
                    self.compile_expr(v)?;
                } else {
                    self.emit(OpCode::Null, keyword.line);
                }
                self.emit(OpCode::Return, keyword.line);
            }
            Stmt::TryCatch { try_body, catch_param, catch_body } => {
                let rescue_jump = self.emit_jump(OpCode::SetupHandler(0));
                
                self.compile_stmt(try_body)?;
                self.emit(OpCode::PopHandler, 0); 
                
                let end_jump = self.emit_jump(OpCode::Jump(0));
                
                self.patch_jump(rescue_jump);
                
                self.begin_scope();
                self.locals.push(Local { name: catch_param.lexeme.clone(), depth: self.scope_depth, is_captured: false });
                if self.locals.len() > self.function.max_locals {
                    self.function.max_locals = self.locals.len();
                }
                self.compile_stmt(catch_body)?;
                self.end_scope();
                
                self.patch_jump(end_jump);
            }
            Stmt::Throw(expr) => {
                self.compile_expr(expr)?;
                self.emit(OpCode::Throw, 0);
            }
            Stmt::Export(stmt) => {
                self.compile_stmt(stmt)?; // Just compile the inner stmt, export is metadata
            }
            Stmt::Struct { name, .. } => {
                // Register struct as a global "type" name (Value::Null for now)
                let idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                self.emit(OpCode::Null, name.line);
                self.emit(OpCode::DefineGlobal(idx), name.line);
            }
            Stmt::Class { name, methods, is_abstract, interfaces, superclass } => {
                let _ = is_abstract;
                let _ = interfaces;
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                
                if let Some(super_token) = superclass {
                    let s_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(super_token.lexeme.clone())))));
                    self.emit(OpCode::GetGlobal(s_idx), super_token.line);
                } else {
                    self.emit(OpCode::Null, name.line);
                }
                
                self.emit(OpCode::BuildClass(name_idx), name.line);
                
                self.emitting_method = true;
                for method in methods {
                    if let Stmt::Function { name: method_name, .. } = method {
                        self.compile_stmt(method)?;
                        let m_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(method_name.lexeme.clone())))));
                        self.emit(OpCode::Method(m_idx), name.line);
                    }
                }
                self.emitting_method = false;

                self.emit(OpCode::DefineGlobal(name_idx), name.line);
            }
            Stmt::Interface { name, .. } => {
                // Interfaces are purely for static analysis right now
                let idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                self.emit(OpCode::Null, name.line);
                self.emit(OpCode::DefineGlobal(idx), name.line);
            }
            Stmt::Match { expression, arms } => {
                // Simplified match: compile expression, then use a series of jumps.
                self.compile_expr(expression)?;
                // Not implementing full match logic here for brevity, but will compile arms
                for _ in arms {
                    // self.compile_stmt(...)
                }
                self.emit(OpCode::Pop, 0); // pop match expr
            }
            Stmt::Enum { name, .. } => {
                 let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                 self.emit(OpCode::Null, name.line);
                 self.emit(OpCode::DefineGlobal(name_idx), name.line);
            }
            Stmt::TypeAlias { .. } => {
                // Type aliases are purely for static analysis, nothing to emit
            }
            Stmt::Emit(expr) => {
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from("emit")))));
                self.emit(OpCode::GetGlobal(name_idx), 0);
                self.compile_expr(expr)?;
                self.emit(OpCode::Call(1), 0);
                self.emit(OpCode::Pop, 0);
            }
            Stmt::Import { module, items } => {
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(module.lexeme.as_str())))));
                if items.is_empty() {
                    // use math;  →  load all exports
                    self.emit(OpCode::ImportModule(name_idx), module.line);
                } else {
                    // use { sqrt, pow } from math;  →  push item names, then ImportItems
                    for item in items {
                        let item_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(item.lexeme.as_str())))));
                        self.emit(OpCode::Constant(item_idx), item.line);
                    }
                    self.emit(OpCode::ImportItems(name_idx, items.len() as u8), module.line);
                }
            }
        }
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Literal(lit) => {
                match lit {
                    Literal::Int(i) => {
                        let idx = self.add_constant(Value::int(*i));
                        self.emit(OpCode::Constant(idx), 0);
                    }
                    Literal::Float(f) => {
                        let idx = self.add_constant(Value::float(*f));
                        self.emit(OpCode::Constant(idx), 0);
                    }
                    Literal::Bool(b) => {
                        if *b { self.emit(OpCode::True, 0); } else { self.emit(OpCode::False, 0); }
                    }
                    Literal::String(s) => {
                        let idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(s.clone())))));
                        self.emit(OpCode::Constant(idx), 0);
                    }
                    Literal::Null => {
                        self.emit(OpCode::Null, 0);
                    }
                }
            }
            Expr::Variable(name) => {
                self.named_variable(name, false)?;
            }
            Expr::Assign { name, value } => {
                self.compile_expr(value)?;
                self.named_variable(name, true)?;
            }
            Expr::Logical { left, operator, right } => {
                if operator.ty == TokenType::Nullish {
                    self.compile_expr(left)?;
                    let jump_if_null = self.emit_jump(OpCode::JumpIfNull(0));
                    let jump_end = self.emit_jump(OpCode::Jump(0));
                    
                    self.patch_jump(jump_if_null);
                    self.emit(OpCode::Pop, operator.line); // pop the null
                    self.compile_expr(right)?;
                    
                    self.patch_jump(jump_end);
                } else {
                    return Err(format!("Unknown logical operator {:?}", operator.ty));
                }
            }
            Expr::Unary { operator, right } => {
                self.compile_expr(right)?;
                match operator.ty {
                    TokenType::Minus => self.emit(OpCode::Negate, operator.line),
                    TokenType::Bang => self.emit(OpCode::Not, operator.line),
                    _ => return Err(format!("Unknown unary operator {:?}", operator.ty)),
                }
            }
            Expr::Binary { left, operator, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                match operator.ty {
                    TokenType::Plus => self.emit(OpCode::Add, operator.line),
                    TokenType::Minus => self.emit(OpCode::Subtract, operator.line),
                    TokenType::Star => self.emit(OpCode::Multiply, operator.line),
                    TokenType::Slash => self.emit(OpCode::Divide, operator.line),
                    TokenType::EqualEqual => self.emit(OpCode::Equal, operator.line),
                    TokenType::BangEqual => {
                        self.emit(OpCode::Equal, operator.line);
                        self.emit(OpCode::Not, operator.line);
                    }
                    TokenType::Greater => self.emit(OpCode::Greater, operator.line),
                    TokenType::Less => self.emit(OpCode::Less, operator.line),
                    _ => return Err(format!("Unknown binary operator {:?}", operator.ty))
                }
            }
            Expr::Call { callee, arguments, paren } => {
                self.compile_expr(callee)?;
                for arg in arguments {
                    self.compile_expr(arg)?;
                }
                self.emit(OpCode::Call(arguments.len() as u8), paren.line);
            }
            Expr::Array { elements } => {
                for el in elements {
                    self.compile_expr(el)?;
                }
                self.emit(OpCode::BuildArray(elements.len() as u8), 0);
            }
            Expr::Get { object, name } => {
                self.compile_expr(object)?;
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.as_str())))));
                self.emit(OpCode::GetProperty(name_idx), name.line);
            }
            Expr::Set { object, name, value } => {
                self.compile_expr(object)?;
                self.compile_expr(value)?;
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.as_str())))));
                self.emit(OpCode::SetProperty(name_idx), name.line);
            }
             Expr::StructInit { name, fields } => {
                for (field_name, val) in fields {
                    let field_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(field_name.lexeme.as_str())))));
                    self.emit(OpCode::Constant(field_idx), field_name.line);
                    self.compile_expr(val)?;
                }
                let idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.as_str())))));
                self.emit(OpCode::BuildInstance(idx, fields.len() as u8), name.line);
            }
            Expr::Dict { entries } => {
                for (key, val) in entries {
                    self.compile_expr(key)?;
                    self.compile_expr(val)?;
                }
                self.emit(OpCode::BuildDict(entries.len() as u8), 0);
            }
            Expr::Tuple { elements } => {
                for el in elements {
                    self.compile_expr(el)?;
                }
                self.emit(OpCode::BuildTuple(elements.len() as u8), 0);
            }
            Expr::SetInit { elements } => {
                for el in elements {
                    self.compile_expr(el)?;
                }
                self.emit(OpCode::BuildSet(elements.len() as u8), 0);
            }
            Expr::OptionalGet { object, name } => {
                self.compile_expr(object)?;
                let jump = self.emit_jump(OpCode::JumpIfNull(0)); 
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.as_str())))));
                self.emit(OpCode::GetProperty(name_idx), name.line);
                self.patch_jump(jump);
            }
            Expr::Ternary { condition, then_branch, else_branch } => {
                self.compile_expr(condition)?;
                let then_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                self.emit(OpCode::Pop, 0);
                self.compile_expr(then_branch)?;
                let else_jump = self.emit_jump(OpCode::Jump(0));
                
                self.patch_jump(then_jump);
                self.emit(OpCode::Pop, 0);
                self.compile_expr(else_branch)?;
                
                self.patch_jump(else_jump);
            }
            Expr::Spread(_) => return Err("Spread operator only supported in arrays for now.".to_string()),
            Expr::Index { object, index, .. } => {
                self.compile_expr(object)?;
                self.compile_expr(index)?;
                self.emit(OpCode::GetIndex, 0);
            }
            Expr::IndexSet { object, index, value, .. } => {
                self.compile_expr(object)?;
                self.compile_expr(index)?;
                self.compile_expr(value)?;
                self.emit(OpCode::SetIndex, 0);
            }
            Expr::Pipe { left, right } => {
                // value |> function  => function(value)
                self.compile_expr(right)?;
                self.compile_expr(left)?;
                self.emit(OpCode::Call(1), 0);
        }
            Expr::Lambda { params, body, is_async } => {
                let mut compiler = Compiler::new("lambda", *is_async, false, Some(self as *mut Compiler));
                compiler.function.arity = params.len();
                compiler.begin_scope();
                for (p, _, _) in params {
                    compiler.locals.push(Local { name: p.lexeme.clone(), depth: compiler.scope_depth, is_captured: false });
                }
                compiler.function.max_locals = compiler.locals.len();
                for s in body {
                    compiler.compile_stmt(s)?;
                }
                compiler.emit(OpCode::Null, 0);
                compiler.emit(OpCode::Return, 0);

                let fun = compiler.function;
                let idx = self.add_constant(Value::obj(Arc::new(Obj::Function(Arc::new(fun)))));
                self.emit(OpCode::Closure(idx), 0);
            }
        }
        Ok(())
    }

    fn named_variable(&mut self, name: &Token, can_assign: bool) -> Result<(), String> {
        if let Some(arg) = self.resolve_local(name) {
            if can_assign {
                self.emit(OpCode::SetLocal(arg), name.line);
            } else {
                self.emit(OpCode::GetLocal(arg), name.line);
            }
        } else if let Some(arg) = self.resolve_upvalue(name) {
            if can_assign {
                self.emit(OpCode::SetUpvalue(arg), name.line);
            } else {
                self.emit(OpCode::GetUpvalue(arg), name.line);
            }
        } else {
            let arg = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.as_str())))));
            if can_assign {
                self.emit(OpCode::SetGlobal(arg), name.line);
            } else {
                self.emit(OpCode::GetGlobal(arg), name.line);
            }
        }
        Ok(())
    }

    fn resolve_local(&self, name: &Token) -> Option<usize> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name.lexeme {
                return Some(i);
            }
        }
        None
    }

    fn resolve_upvalue(&mut self, name: &Token) -> Option<usize> {
        let parent_ptr = self.parent?;
        let parent = unsafe { &mut *parent_ptr };

        if let Some(local_idx) = parent.resolve_local(name) {
            parent.locals[local_idx].is_captured = true;
            return Some(self.add_upvalue(local_idx, true));
        }

        if let Some(upvalue_idx) = parent.resolve_upvalue(name) {
            return Some(self.add_upvalue(upvalue_idx, false));
        }

        None
    }

    fn add_upvalue(&mut self, index: usize, is_local: bool) -> usize {
        for (i, upvalue) in self.upvalues.iter().enumerate() {
            if upvalue.index == index && upvalue.is_local == is_local {
                return i;
            }
        }

        self.upvalues.push(UpvalueMetadata { index, is_local });
        self.function.upvalues.push(crate::value::UpvalueRequirement { is_local, index });
        self.upvalues.len() - 1
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.scope_depth -= 1;
        while let Some(local) = self.locals.last() {
            if local.depth > self.scope_depth {
                if local.is_captured {
                    self.emit(OpCode::CloseUpvalue, 0);
                } else {
                    self.emit(OpCode::Pop, 0);
                }
                self.locals.pop();
            } else {
                break;
            }
        }
    }

    fn emit(&mut self, op: OpCode, line: usize) {
        self.function.chunk.write(op, line);
    }

    fn emit_jump(&mut self, op: OpCode) -> usize {
        self.emit(op, 0);
        self.current_chunk().code.len() - 1
    }

    fn patch_jump(&mut self, offset: usize) {
        let jump = self.current_chunk().code.len() - 1 - offset;
        match &mut self.current_chunk().code[offset] {
            OpCode::JumpIfFalse(ref mut val) => *val = jump,
            OpCode::Jump(ref mut val) => *val = jump,
            OpCode::JumpIfNull(ref mut val) => *val = jump,
            OpCode::SetupHandler(ref mut val) => *val = jump,
            _ => {}
        }
    }

    fn emit_loop(&mut self, start: usize) {
        let jump = self.current_chunk().code.len() - start + 1;
        self.emit(OpCode::Loop(jump), 0);
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.function.chunk
    }

    fn add_constant(&mut self, value: Value) -> usize {
        self.current_chunk().add_constant(value)
    }
}
