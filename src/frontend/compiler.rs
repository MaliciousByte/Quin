use std::collections::HashMap;
use crate::frontend::ast::{Expr, Stmt, Literal, Type};
use crate::frontend::chunk::{Chunk, TypeTag, encode_inst, encode_inst_imm16, decode_inst_imm16};
use crate::frontend::chunk::{
    OP_LOAD_CONST, OP_LOAD_NULL, OP_LOAD_TRUE, OP_LOAD_FALSE, OP_MOVE,
    OP_GET_GLOBAL, OP_SET_GLOBAL, OP_DEFINE_GLOBAL, OP_EQUAL, OP_GREATER,
    OP_LESS, OP_ADD, OP_SUBTRACT, OP_MULTIPLY, OP_DIVIDE, OP_NOT, OP_NEGATE,
    OP_JUMP_IF_FALSE, OP_JUMP, OP_LOOP, OP_CALL, OP_RETURN, OP_BUILD_ARRAY,
    OP_BUILD_DICT, OP_BUILD_TUPLE, OP_BUILD_SET, OP_GET_INDEX, OP_SET_INDEX,
    OP_BUILD_INSTANCE, OP_GET_PROPERTY, OP_SET_PROPERTY, OP_THROW, OP_BUILD_CLASS,
    OP_METHOD, OP_JUMP_IF_NULL, OP_CLOSURE, OP_GET_UPVALUE, OP_SET_UPVALUE,
    OP_CLOSE_UPVALUE, OP_SETUP_HANDLER, OP_POP_HANDLER, OP_IMPORT_MODULE,
    OP_IMPORT_ITEMS, OP_NEQ,
};
use crate::frontend::token::{Token, TokenType};
use crate::value::{Value, Function};
use crate::vm::obj::Obj;
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

    pub type_metadata: Vec<Option<TypeTag>>,
    pub mutability_flags: Vec<bool>,
    pub max_registers: usize,
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
            type_metadata: Vec::new(),
            mutability_flags: Vec::new(),
            max_registers: 1, // slot 0 for closure/self
        };
        // slot 0 for local call frame
        let slot0_name = if is_method { "self".to_string() } else { "".to_string() };
        compiler.locals.push(Local { name: slot0_name, depth: 0, is_captured: false });
        compiler.type_metadata.push(None);
        compiler.mutability_flags.push(false);
        compiler
    }

    fn update_max_registers(&mut self, reg: u8) {
        let r = (reg as usize) + 1;
        if r > self.max_registers {
            self.max_registers = r;
        }
    }

    pub fn compile(mut self, stmts: &[Stmt]) -> Result<Function, String> {
        for stmt in stmts {
            self.compile_stmt(stmt)?;
        }
        
        // Return null at the end by default if not returned
        let res_reg = self.locals.len() as u8;
        self.update_max_registers(res_reg);
        self.emit_inst(encode_inst(OP_LOAD_NULL, res_reg, 0, 0), 0);
        self.emit_inst(encode_inst(OP_RETURN, res_reg, 0, 0), 0);

        self.finish()
    }

    fn finish(mut self) -> Result<Function, String> {
        // Resize type metadata to match register count
        self.function.chunk.register_count = self.max_registers as u32;
        self.function.max_locals = self.max_registers;

        let mut type_meta = self.type_metadata;
        type_meta.resize(self.max_registers, None);
        let mut mut_flags = self.mutability_flags;
        mut_flags.resize(self.max_registers, false);

        self.function.chunk.type_metadata = type_meta;
        self.function.chunk.mutability_flags = mut_flags;
        self.function.chunk.observed_types = vec![None; self.max_registers];

        // Compute bytecode_hash before returning
        let mut h1: u64 = 0xcbf29ce484222325;
        let mut h2: u64 = 0x811c9dc5;
        for &val in &self.function.chunk.code {
            for byte in val.to_ne_bytes() {
                h1 ^= byte as u64;
                h1 = h1.wrapping_mul(0x100000001b3);
                h2 ^= byte as u64;
                h2 = h2.wrapping_mul(1099511628211);
            }
        }
        let mut hash = [0u8; 16];
        hash[0..8].copy_from_slice(&h1.to_le_bytes());
        hash[8..16].copy_from_slice(&h2.to_le_bytes());
        self.function.chunk.bytecode_hash = hash;

        Ok(self.function)
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Expression(expr) => {
                let slot = self.locals.len() as u8;
                self.update_max_registers(slot);
                self.compile_expr(expr, slot)?;
            }

            Stmt::Let { pattern, is_mut, type_annotation, initializer } => {
                let slot = self.locals.len();
                if let Some(init) = initializer {
                    self.compile_expr(init, slot as u8)?;
                } else {
                    self.update_max_registers(slot as u8);
                    self.emit_inst(encode_inst(OP_LOAD_NULL, slot as u8, 0, 0), 0);
                }

                // Handle type tag
                let mut tag = None;
                if let Some(Type::Simple(ref s)) = type_annotation {
                    if s == "int" {
                        tag = Some(TypeTag::Int);
                    } else if s == "float" {
                        tag = Some(TypeTag::Float);
                    } else if s == "bool" {
                        tag = Some(TypeTag::Bool);
                    } else if s == "string" {
                        tag = Some(TypeTag::String);
                    }
                }

                // Grow metadata if needed
                while self.type_metadata.len() <= slot {
                    self.type_metadata.push(None);
                    self.mutability_flags.push(false);
                }

                if *is_mut {
                    self.type_metadata[slot] = None;
                    self.mutability_flags[slot] = true;
                } else {
                    self.type_metadata[slot] = tag;
                    self.mutability_flags[slot] = false;
                }

                match pattern {
                    crate::frontend::ast::Pattern::Identifier(name) => {
                        if self.scope_depth > 0 {
                            self.locals.push(Local {
                                name: name.lexeme.clone(),
                                depth: self.scope_depth,
                                is_captured: false,
                            });
                            self.update_max_registers((self.locals.len() - 1) as u8);
                        } else {
                            let idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                            self.emit_inst(encode_inst_imm16(OP_DEFINE_GLOBAL, slot as u8, idx as u16), name.line);
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
                let cond_reg = self.locals.len() as u8;
                self.update_max_registers(cond_reg);
                self.compile_expr(condition, cond_reg)?;

                let then_jump = self.emit_jump(OP_JUMP_IF_FALSE, cond_reg);
                self.compile_stmt(then_branch)?;
                
                let mut end_jumps = vec![self.emit_jump(OP_JUMP, 0)];
                self.patch_jump(then_jump);

                for (elif_cond, elif_body) in elif_branches {
                    self.compile_expr(elif_cond, cond_reg)?;
                    let elif_jump = self.emit_jump(OP_JUMP_IF_FALSE, cond_reg);
                    self.compile_stmt(elif_body)?;
                    end_jumps.push(self.emit_jump(OP_JUMP, 0));
                    self.patch_jump(elif_jump);
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
                let cond_reg = self.locals.len() as u8;
                self.update_max_registers(cond_reg);
                self.compile_expr(condition, cond_reg)?;

                let exit_jump = self.emit_jump(OP_JUMP_IF_FALSE, cond_reg);
                self.compile_stmt(body)?;
                self.emit_loop(loop_start);
                self.patch_jump(exit_jump);
            }

            Stmt::For { .. } => {
                return Err("For loops require iterator protocol or array length logic via stdlib, simplifying for VM v1.".to_string());
            }

            Stmt::Function { name, params, variadic: _, is_async, is_static: _, is_abstract: _, visibility: _, return_type: _, body } => {
                let is_method = self.emitting_method;
                let mut compiler = Compiler::new(&name.lexeme, *is_async, is_method, Some(self as *mut Compiler));
                compiler.function.arity = params.len();

                compiler.begin_scope();
                for (p, type_annotation, _) in params {
                    let slot = compiler.locals.len();
                    let mut tag = None;
                    if let Some(Type::Simple(ref s)) = type_annotation {
                        if s == "int" { tag = Some(TypeTag::Int); }
                        else if s == "float" { tag = Some(TypeTag::Float); }
                        else if s == "bool" { tag = Some(TypeTag::Bool); }
                        else if s == "string" { tag = Some(TypeTag::String); }
                    }
                    while compiler.type_metadata.len() <= slot {
                        compiler.type_metadata.push(None);
                        compiler.mutability_flags.push(false);
                    }
                    compiler.type_metadata[slot] = tag;
                    compiler.mutability_flags[slot] = false;
                    compiler.locals.push(Local { name: p.lexeme.clone(), depth: compiler.scope_depth, is_captured: false });
                }
                compiler.max_registers = compiler.locals.len();

                for s in body {
                    compiler.compile_stmt(s)?;
                }

                if name.lexeme == "init" || name.lexeme == "constructor" {
                    // return self (slot 0)
                    compiler.emit_inst(encode_inst(OP_RETURN, 0, 0, 0), 0);
                } else {
                    let res_reg = compiler.locals.len() as u8;
                    compiler.update_max_registers(res_reg);
                    compiler.emit_inst(encode_inst(OP_LOAD_NULL, res_reg, 0, 0), 0);
                    compiler.emit_inst(encode_inst(OP_RETURN, res_reg, 0, 0), 0);
                }

                let fun = compiler.finish()?;
                let idx = self.add_constant(Value::obj(Arc::new(Obj::Function(Arc::new(fun)))));

                if self.emitting_method {
                    let dest_reg = self.locals.len() as u8;
                    self.update_max_registers(dest_reg);
                    self.emit_inst(encode_inst_imm16(OP_CLOSURE, dest_reg, idx as u16), name.line);
                } else if self.scope_depth > 0 {
                    let dest_reg = self.locals.len() as u8;
                    self.update_max_registers(dest_reg);
                    self.emit_inst(encode_inst_imm16(OP_CLOSURE, dest_reg, idx as u16), name.line);
                    self.locals.push(Local { name: name.lexeme.clone(), depth: self.scope_depth, is_captured: false });
                    self.update_max_registers((self.locals.len() - 1) as u8);
                } else {
                    let temp_reg = self.locals.len() as u8;
                    self.update_max_registers(temp_reg);
                    let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                    self.emit_inst(encode_inst_imm16(OP_CLOSURE, temp_reg, idx as u16), name.line);
                    self.emit_inst(encode_inst_imm16(OP_DEFINE_GLOBAL, temp_reg, name_idx as u16), name.line);
                }
            }

            Stmt::Return { keyword, value } => {
                let res_reg = self.locals.len() as u8;
                self.update_max_registers(res_reg);
                if let Some(v) = value {
                    self.compile_expr(v, res_reg)?;
                } else {
                    self.emit_inst(encode_inst(OP_LOAD_NULL, res_reg, 0, 0), keyword.line);
                }
                self.emit_inst(encode_inst(OP_RETURN, res_reg, 0, 0), keyword.line);
            }

            Stmt::TryCatch { try_body, catch_param, catch_body } => {
                let catch_param_reg = self.locals.len() as u8;
                self.update_max_registers(catch_param_reg);
                let rescue_jump = self.emit_jump(OP_SETUP_HANDLER, catch_param_reg);

                self.compile_stmt(try_body)?;
                self.emit_inst(encode_inst(OP_POP_HANDLER, 0, 0, 0), 0);
                
                let end_jump = self.emit_jump(OP_JUMP, 0);
                self.patch_jump(rescue_jump);

                self.begin_scope();
                self.locals.push(Local { name: catch_param.lexeme.clone(), depth: self.scope_depth, is_captured: false });
                self.update_max_registers((self.locals.len() - 1) as u8);
                self.compile_stmt(catch_body)?;
                self.end_scope();

                self.patch_jump(end_jump);
            }

            Stmt::Throw(expr) => {
                let throw_reg = self.locals.len() as u8;
                self.update_max_registers(throw_reg);
                self.compile_expr(expr, throw_reg)?;
                self.emit_inst(encode_inst(OP_THROW, throw_reg, 0, 0), 0);
            }

            Stmt::Export(stmt) => {
                self.compile_stmt(stmt)?;
            }

            Stmt::Struct { name, .. } => {
                let idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                let temp_reg = self.locals.len() as u8;
                self.update_max_registers(temp_reg);
                self.emit_inst(encode_inst(OP_LOAD_NULL, temp_reg, 0, 0), name.line);
                self.emit_inst(encode_inst_imm16(OP_DEFINE_GLOBAL, temp_reg, idx as u16), name.line);
            }

            Stmt::Class { name, methods, is_abstract: _, interfaces: _, superclass } => {
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                let dest_reg = self.locals.len() as u8;
                self.update_max_registers(dest_reg + 1);

                // Load class name as string constant to dest_reg
                self.emit_inst(encode_inst_imm16(OP_LOAD_CONST, dest_reg, name_idx as u16), name.line);

                // Superclass to dest_reg + 1
                let super_reg = dest_reg + 1;
                if let Some(super_token) = superclass {
                    self.named_variable(super_token, false, super_reg)?;
                } else {
                    self.emit_inst(encode_inst(OP_LOAD_NULL, super_reg, 0, 0), name.line);
                }

                // Reserve dest_reg and super_reg
                self.locals.push(Local { name: "".to_string(), depth: self.scope_depth, is_captured: false });
                self.locals.push(Local { name: "".to_string(), depth: self.scope_depth, is_captured: false });

                self.emit_inst(encode_inst(OP_BUILD_CLASS, dest_reg, super_reg, 0), name.line);

                self.emitting_method = true;
                for method in methods {
                    if let Stmt::Function { name: method_name, .. } = method {
                        let method_reg = (self.locals.len()) as u8;
                        self.update_max_registers(method_reg + 1); // for method name
                        self.compile_stmt(method)?;
                        
                        let m_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(method_name.lexeme.clone())))));
                        let name_reg = method_reg + 1;
                        self.emit_inst(encode_inst_imm16(OP_LOAD_CONST, name_reg, m_idx as u16), name.line);
                        
                        self.emit_inst(encode_inst(OP_METHOD, dest_reg, name_reg, method_reg), name.line);
                    }
                }
                self.emitting_method = false;

                self.emit_inst(encode_inst_imm16(OP_DEFINE_GLOBAL, dest_reg, name_idx as u16), name.line);

                self.locals.pop();
                self.locals.pop();
            }

            Stmt::Interface { name, .. } => {
                let idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                let temp_reg = self.locals.len() as u8;
                self.update_max_registers(temp_reg);
                self.emit_inst(encode_inst(OP_LOAD_NULL, temp_reg, 0, 0), name.line);
                self.emit_inst(encode_inst_imm16(OP_DEFINE_GLOBAL, temp_reg, idx as u16), name.line);
            }

            Stmt::Match { .. } => {
                return Err("Match statements not fully supported in compiler.".to_string());
            }

            Stmt::Enum { name, .. } => {
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.clone())))));
                let temp_reg = self.locals.len() as u8;
                self.update_max_registers(temp_reg);
                self.emit_inst(encode_inst(OP_LOAD_NULL, temp_reg, 0, 0), name.line);
                self.emit_inst(encode_inst_imm16(OP_DEFINE_GLOBAL, temp_reg, name_idx as u16), name.line);
            }

            Stmt::TypeAlias { .. } => {}

            Stmt::Emit(expr) => {
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from("emit")))));
                let temp_reg = self.locals.len() as u8;
                self.update_max_registers(temp_reg + 1);
                
                self.emit_inst(encode_inst_imm16(OP_GET_GLOBAL, temp_reg, name_idx as u16), 0);
                self.compile_expr(expr, temp_reg + 1)?;
                self.emit_inst(encode_inst(OP_CALL, temp_reg, temp_reg, 1), 0);
            }

            Stmt::Import { module, items } => {
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(module.lexeme.as_str())))));
                if items.is_empty() {
                    self.emit_inst(encode_inst_imm16(OP_IMPORT_MODULE, 0, name_idx as u16), module.line);
                } else {
                    let name_reg = self.locals.len() as u8;
                    self.update_max_registers(name_reg + items.len() as u8);
                    self.emit_inst(encode_inst_imm16(OP_LOAD_CONST, name_reg, name_idx as u16), module.line);
                    
                    let start_reg = name_reg + 1;
                    for (i, item) in items.iter().enumerate() {
                        let item_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(item.lexeme.as_str())))));
                        self.emit_inst(encode_inst_imm16(OP_LOAD_CONST, start_reg + i as u8, item_idx as u16), item.line);
                    }
                    
                    self.emit_inst(encode_inst(OP_IMPORT_ITEMS, name_reg, start_reg, items.len() as u8), module.line);
                }
            }
        }
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr, dest_reg: u8) -> Result<(), String> {
        self.update_max_registers(dest_reg);
        match expr {
            Expr::Literal(lit) => {
                match lit {
                    Literal::Int(i) => {
                        let idx = self.add_constant(Value::int(*i));
                        self.emit_inst(encode_inst_imm16(OP_LOAD_CONST, dest_reg, idx as u16), 0);
                    }
                    Literal::Float(f) => {
                        let idx = self.add_constant(Value::float(*f));
                        self.emit_inst(encode_inst_imm16(OP_LOAD_CONST, dest_reg, idx as u16), 0);
                    }
                    Literal::Bool(b) => {
                        let op = if *b { OP_LOAD_TRUE } else { OP_LOAD_FALSE };
                        self.emit_inst(encode_inst(op, dest_reg, 0, 0), 0);
                    }
                    Literal::String(s) => {
                        let idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(s.clone())))));
                        self.emit_inst(encode_inst_imm16(OP_LOAD_CONST, dest_reg, idx as u16), 0);
                    }
                    Literal::Null => {
                        self.emit_inst(encode_inst(OP_LOAD_NULL, dest_reg, 0, 0), 0);
                    }
                }
            }

            Expr::Variable(name) => {
                self.named_variable(name, false, dest_reg)?;
            }

            Expr::Assign { name, value } => {
                self.compile_expr(value, dest_reg)?;
                self.named_variable(name, true, dest_reg)?;
            }

            Expr::Logical { left, operator, right } => {
                if operator.ty == TokenType::Nullish {
                    self.compile_expr(left, dest_reg)?;
                    let jump_if_null = self.emit_jump(OP_JUMP_IF_NULL, dest_reg);
                    let jump_end = self.emit_jump(OP_JUMP, 0);
                    
                    self.patch_jump(jump_if_null);
                    self.compile_expr(right, dest_reg)?;
                    self.patch_jump(jump_end);
                } else {
                    return Err(format!("Unknown logical operator {:?}", operator.ty));
                }
            }

            Expr::Unary { operator, right } => {
                self.compile_expr(right, dest_reg)?;
                match operator.ty {
                    TokenType::Minus => self.emit_inst(encode_inst(OP_NEGATE, dest_reg, dest_reg, 0), operator.line),
                    TokenType::Bang => self.emit_inst(encode_inst(OP_NOT, dest_reg, dest_reg, 0), operator.line),
                    _ => return Err(format!("Unknown unary operator {:?}", operator.ty)),
                }
            }

            Expr::Binary { left, operator, right } => {
                self.compile_expr(left, dest_reg)?;
                let temp_reg = dest_reg + 1;
                self.compile_expr(right, temp_reg)?;
                
                let op = match operator.ty {
                    TokenType::Plus => OP_ADD,
                    TokenType::Minus => OP_SUBTRACT,
                    TokenType::Star => OP_MULTIPLY,
                    TokenType::Slash => OP_DIVIDE,
                    TokenType::EqualEqual => OP_EQUAL,
                    TokenType::BangEqual => OP_NEQ,
                    TokenType::Greater => OP_GREATER,
                    TokenType::Less => OP_LESS,
                    _ => return Err(format!("Unknown binary operator {:?}", operator.ty))
                };
                self.emit_inst(encode_inst(op, dest_reg, dest_reg, temp_reg), operator.line);
            }

            Expr::Call { callee, arguments, paren } => {
                // Compile callee to dest_reg, and arguments contiguously starting at dest_reg + 1
                self.compile_expr(callee, dest_reg)?;
                for (i, arg) in arguments.iter().enumerate() {
                    self.compile_expr(arg, dest_reg + 1 + i as u8)?;
                }
                self.emit_inst(encode_inst(OP_CALL, dest_reg, dest_reg, arguments.len() as u8), paren.line);
            }

            Expr::Array { elements } => {
                for (i, el) in elements.iter().enumerate() {
                    self.compile_expr(el, dest_reg + i as u8)?;
                }
                self.emit_inst(encode_inst(OP_BUILD_ARRAY, dest_reg, dest_reg, elements.len() as u8), 0);
            }

            Expr::Get { object, name } => {
                self.compile_expr(object, dest_reg)?;
                let name_reg = dest_reg + 1;
                self.compile_expr(&Expr::Literal(Literal::String(name.lexeme.clone())), name_reg)?;
                self.emit_inst(encode_inst(OP_GET_PROPERTY, dest_reg, dest_reg, name_reg), name.line);
            }

            Expr::Set { object, name, value } => {
                self.compile_expr(object, dest_reg)?;
                let val_reg = dest_reg + 1;
                self.compile_expr(value, val_reg)?;
                
                let name_reg = dest_reg + 2;
                self.compile_expr(&Expr::Literal(Literal::String(name.lexeme.clone())), name_reg)?;
                
                self.emit_inst(encode_inst(OP_SET_PROPERTY, dest_reg, name_reg, val_reg), name.line);
                // Assign value to dest_reg
                self.emit_inst(encode_inst(OP_MOVE, dest_reg, val_reg, 0), name.line);
            }

            Expr::StructInit { name, fields } => {
                let name_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.as_str())))));
                self.emit_inst(encode_inst_imm16(OP_LOAD_CONST, dest_reg, name_idx as u16), name.line);

                let start_reg = dest_reg + 1;
                for (i, (field_name, val)) in fields.iter().enumerate() {
                    let field_idx = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(field_name.lexeme.as_str())))));
                    let name_reg = start_reg + i as u8 * 2;
                    let val_reg = name_reg + 1;
                    
                    self.emit_inst(encode_inst_imm16(OP_LOAD_CONST, name_reg, field_idx as u16), field_name.line);
                    self.compile_expr(val, val_reg)?;
                }
                
                self.emit_inst(encode_inst(OP_BUILD_INSTANCE, dest_reg, start_reg, fields.len() as u8), name.line);
            }

            Expr::Dict { entries } => {
                let start_reg = dest_reg;
                for (i, (key, val)) in entries.iter().enumerate() {
                    self.compile_expr(key, start_reg + i as u8 * 2)?;
                    self.compile_expr(val, start_reg + i as u8 * 2 + 1)?;
                }
                self.emit_inst(encode_inst(OP_BUILD_DICT, dest_reg, start_reg, entries.len() as u8), 0);
            }

            Expr::Tuple { elements } => {
                let start_reg = dest_reg;
                for (i, el) in elements.iter().enumerate() {
                    self.compile_expr(el, start_reg + i as u8)?;
                }
                self.emit_inst(encode_inst(OP_BUILD_TUPLE, dest_reg, start_reg, elements.len() as u8), 0);
            }

            Expr::SetInit { elements } => {
                let start_reg = dest_reg;
                for (i, el) in elements.iter().enumerate() {
                    self.compile_expr(el, start_reg + i as u8)?;
                }
                self.emit_inst(encode_inst(OP_BUILD_SET, dest_reg, start_reg, elements.len() as u8), 0);
            }

            Expr::OptionalGet { object, name } => {
                self.compile_expr(object, dest_reg)?;
                let jump = self.emit_jump(OP_JUMP_IF_NULL, dest_reg);
                
                let name_reg = dest_reg + 1;
                self.compile_expr(&Expr::Literal(Literal::String(name.lexeme.clone())), name_reg)?;
                self.emit_inst(encode_inst(OP_GET_PROPERTY, dest_reg, dest_reg, name_reg), name.line);
                
                self.patch_jump(jump);
            }

            Expr::Ternary { condition, then_branch, else_branch } => {
                self.compile_expr(condition, dest_reg)?;
                let then_jump = self.emit_jump(OP_JUMP_IF_FALSE, dest_reg);
                self.compile_expr(then_branch, dest_reg)?;
                let else_jump = self.emit_jump(OP_JUMP, 0);
                
                self.patch_jump(then_jump);
                self.compile_expr(else_branch, dest_reg)?;
                self.patch_jump(else_jump);
            }

            Expr::Spread(_) => return Err("Spread operator only supported in arrays for now.".to_string()),

            Expr::Index { object, index, .. } => {
                self.compile_expr(object, dest_reg)?;
                let idx_reg = dest_reg + 1;
                self.compile_expr(index, idx_reg)?;
                self.emit_inst(encode_inst(OP_GET_INDEX, dest_reg, dest_reg, idx_reg), 0);
            }

            Expr::IndexSet { object, index, value, .. } => {
                self.compile_expr(object, dest_reg)?;
                let idx_reg = dest_reg + 1;
                self.compile_expr(index, idx_reg)?;
                
                let val_reg = dest_reg + 2;
                self.compile_expr(value, val_reg)?;
                
                self.emit_inst(encode_inst(OP_SET_INDEX, dest_reg, idx_reg, val_reg), 0);
                self.emit_inst(encode_inst(OP_MOVE, dest_reg, val_reg, 0), 0);
            }

            Expr::Pipe { left, right } => {
                self.compile_expr(right, dest_reg)?;
                self.compile_expr(left, dest_reg + 1)?;
                self.emit_inst(encode_inst(OP_CALL, dest_reg, dest_reg, 1), 0);
            }

            Expr::Lambda { params, body, is_async } => {
                let mut compiler = Compiler::new("lambda", *is_async, false, Some(self as *mut Compiler));
                compiler.function.arity = params.len();
                compiler.begin_scope();
                for (p, _, _) in params {
                    compiler.locals.push(Local { name: p.lexeme.clone(), depth: compiler.scope_depth, is_captured: false });
                }
                compiler.max_registers = compiler.locals.len();
                for s in body {
                    compiler.compile_stmt(s)?;
                }
                let res_reg = compiler.locals.len() as u8;
                compiler.update_max_registers(res_reg);
                compiler.emit_inst(encode_inst(OP_LOAD_NULL, res_reg, 0, 0), 0);
                compiler.emit_inst(encode_inst(OP_RETURN, res_reg, 0, 0), 0);

                let fun = compiler.finish()?;
                let idx = self.add_constant(Value::obj(Arc::new(Obj::Function(Arc::new(fun)))));
                self.emit_inst(encode_inst_imm16(OP_CLOSURE, dest_reg, idx as u16), 0);
            }
        }
        Ok(())
    }

    fn named_variable(&mut self, name: &Token, can_assign: bool, dest_reg: u8) -> Result<(), String> {
        if let Some(arg) = self.resolve_local(name) {
            if can_assign {
                self.emit_inst(encode_inst(OP_MOVE, arg as u8, dest_reg, 0), name.line);
            } else {
                self.emit_inst(encode_inst(OP_MOVE, dest_reg, arg as u8, 0), name.line);
            }
        } else if let Some(arg) = self.resolve_upvalue(name) {
            if can_assign {
                self.emit_inst(encode_inst(OP_SET_UPVALUE, dest_reg, arg as u8, 0), name.line);
            } else {
                self.emit_inst(encode_inst(OP_GET_UPVALUE, dest_reg, arg as u8, 0), name.line);
            }
        } else {
            let arg = self.add_constant(Value::obj(Arc::new(Obj::String(Arc::from(name.lexeme.as_str())))));
            if can_assign {
                self.emit_inst(encode_inst_imm16(OP_SET_GLOBAL, dest_reg, arg as u16), name.line);
            } else {
                self.emit_inst(encode_inst_imm16(OP_GET_GLOBAL, dest_reg, arg as u16), name.line);
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
                let reg = (self.locals.len() - 1) as u8;
                if local.is_captured {
                    self.emit_inst(encode_inst(OP_CLOSE_UPVALUE, reg, 0, 0), 0);
                }
                self.locals.pop();
            } else {
                break;
            }
        }
    }

    fn emit_inst(&mut self, inst: u32, line: usize) {
        self.function.chunk.write(inst, line);
    }

    fn emit_jump(&mut self, op: u8, reg: u8) -> usize {
        let inst = encode_inst_imm16(op, reg, 0);
        self.emit_inst(inst, 0);
        self.current_chunk().code.len() - 1
    }

    fn patch_jump(&mut self, offset: usize) {
        let jump = (self.current_chunk().code.len() - 1 - offset) as u16;
        let inst = &mut self.current_chunk().code[offset];
        let (op, reg, _) = decode_inst_imm16(*inst);
        *inst = encode_inst_imm16(op, reg, jump);
    }

    fn emit_loop(&mut self, start: usize) {
        let jump = (self.current_chunk().code.len() - start + 1) as u16;
        let inst = encode_inst_imm16(OP_LOOP, 0, jump);
        self.emit_inst(inst, 0);
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.function.chunk
    }

    fn add_constant(&mut self, value: Value) -> usize {
        self.current_chunk().add_constant(value)
    }
}
