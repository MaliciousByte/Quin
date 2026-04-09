use crate::token::{Token, TokenType};
use crate::ast::{Expr, Stmt, Literal, Type, Pattern, Visibility, MatchArm};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, String> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            statements.push(self.declaration()?);
        }
        Ok(statements)
    }

    fn declaration(&mut self) -> Result<Stmt, String> {
        let mut visibility = Visibility::Public;
        if self.match_token(&[TokenType::Pub]) {
            visibility = Visibility::Public;
        } else if self.match_token(&[TokenType::Priv]) {
            visibility = Visibility::Private;
        }

        if self.match_token(&[TokenType::Export]) {
            let stmt = self.declaration()?;
            return Ok(Stmt::Export(Box::new(stmt)));
        }
        if self.match_token(&[TokenType::Let, TokenType::Const]) {
            if self.previous().ty == TokenType::Const {
                self.current -= 1; // step back to read it smoothly in `let_declaration`
                self.let_declaration()
            } else {
                self.let_declaration()
            }

        } else if self.match_token(&[TokenType::Task]) || self.check(TokenType::Async) || self.check(TokenType::Shared) {
            let is_static = self.match_token(&[TokenType::Shared]);
            let is_async = self.match_token(&[TokenType::Async]);
            if is_async { self.consume(TokenType::Task, "Expect 'task' after 'async'.")?; }
            else { self.match_token(&[TokenType::Task]); }
            self.task_declaration(visibility, is_static, is_async, false)
        } else if self.match_token(&[TokenType::Class]) || self.check(TokenType::Base) {
            let is_abstract = self.match_token(&[TokenType::Base]);
            if is_abstract { self.consume(TokenType::Class, "Expect 'class' after 'base'.")?; }
            else { self.match_token(&[TokenType::Class]); }
            self.class_declaration(is_abstract)
        } else if self.match_token(&[TokenType::Trait]) {
            self.interface_declaration()
        } else if self.match_token(&[TokenType::Raise]) {
            self.throw_statement()
        } else if self.match_token(&[TokenType::Struct]) {
            self.struct_declaration()
        } else if self.match_token(&[TokenType::Use]) {
            self.import_declaration()
        } else if self.match_token(&[TokenType::Enum]) {
            self.enum_declaration()
        } else if self.match_token(&[TokenType::Type]) {
            self.type_alias_declaration()
        } else {
            self.statement()
        }
    }

    fn let_declaration(&mut self) -> Result<Stmt, String> {
        self.match_token(&[TokenType::Mut]); // optional mut keyword
        let is_const = self.match_token(&[TokenType::Const]);
        
        let pattern = if self.match_token(&[TokenType::LeftBrace]) {
            let mut names = Vec::new();
            if !self.check(TokenType::RightBrace) {
                loop {
                    names.push(self.consume(TokenType::Identifier, "Expect variable name in destructuring.")?.clone());
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
            }
            self.consume(TokenType::RightBrace, "Expect '}' after destructuring.")?;
            Pattern::Object(names)
        } else if self.match_token(&[TokenType::LeftBracket]) {
            let mut names = Vec::new();
            if !self.check(TokenType::RightBracket) {
                loop {
                    names.push(self.consume(TokenType::Identifier, "Expect variable name in destructuring.")?.clone());
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
            }
            self.consume(TokenType::RightBracket, "Expect ']' after destructuring.")?;
            Pattern::Array(names)
        } else {
            Pattern::Identifier(self.consume(TokenType::Identifier, "Expect variable name.")?.clone())
        };
        
        let mut type_annotation = None;
        if self.match_token(&[TokenType::Colon]) {
            type_annotation = Some(self.parse_type()?);
        }

        let initializer = if self.match_token(&[TokenType::Equal]) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(TokenType::Semicolon, "Expect ';' after variable declaration.")?;
        Ok(Stmt::Let { pattern, is_const, type_annotation, initializer })
    }

    fn parse_params(&mut self) -> Result<Vec<(Token, Option<Type>, Option<Expr>)>, String> {
        let mut params = Vec::new();
        if !self.check(TokenType::RightParen) {
            loop {
                if self.match_token(&[TokenType::DotDotDot]) {
                    let param_name = self.consume(TokenType::Identifier, "Expect parameter name after '...'.")?.clone();
                    let param_type = if self.match_token(&[TokenType::Colon]) { Some(self.parse_type()?) } else { None };
                    params.push((param_name, param_type, None));
                    break;
                }
                let param_name = self.consume(TokenType::Identifier, "Expect parameter name.")?.clone();
                let param_type = if self.match_token(&[TokenType::Colon]) { Some(self.parse_type()?) } else { None };
                let mut default_value = None;
                if self.match_token(&[TokenType::Equal]) {
                    default_value = Some(self.expression()?);
                }
                params.push((param_name, param_type, default_value));
                if !self.match_token(&[TokenType::Comma]) { break; }
            }
        }
        Ok(params)
    }

    fn task_declaration(&mut self, visibility: Visibility, is_static: bool, is_async: bool, is_abstract: bool) -> Result<Stmt, String> {
        let name = self.consume(TokenType::Identifier, "Expect function name.")?.clone();
        self.consume(TokenType::LeftParen, "Expect '(' after function name.")?;
        
        let params = self.parse_params()?;
        self.consume(TokenType::RightParen, "Expect ')' after parameters.")?;

        let return_type = if self.match_token(&[TokenType::Arrow]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = if is_abstract || self.match_token(&[TokenType::Semicolon]) {
            if !is_abstract { self.match_token(&[TokenType::Semicolon]); } // Consume if it was just a semicolon header
            else { self.consume(TokenType::Semicolon, "Expect ';' after base task.")?; }
            Vec::new()
        } else if self.match_token(&[TokenType::FatArrow]) {
            let expr = self.expression()?;
            self.consume(TokenType::Semicolon, "Expect ';' after arrow function body.")?;
            vec![Stmt::Return { keyword: name.clone(), value: Some(expr) }]
        } else {
            self.consume(TokenType::LeftBrace, "Expect '{' before function body.")?;
            self.block()?
        };

        Ok(Stmt::Function { name, params, variadic: false, is_async, is_static, is_abstract, visibility, return_type, body })
    }

    fn enum_declaration(&mut self) -> Result<Stmt, String> {
        let name = self.consume(TokenType::Identifier, "Expect enum name.")?.clone();
        self.consume(TokenType::LeftBrace, "Expect '{' before enum body.")?;
        let mut variants = Vec::new();
        if !self.check(TokenType::RightBrace) {
            loop {
                variants.push(self.consume(TokenType::Identifier, "Expect variant name.")?.clone());
                if !self.match_token(&[TokenType::Comma]) { break; }
            }
        }
        self.consume(TokenType::RightBrace, "Expect '}' after enum variants.")?;
        Ok(Stmt::Enum { name, variants })
    }

    fn type_alias_declaration(&mut self) -> Result<Stmt, String> {
        let name = self.consume(TokenType::Identifier, "Expect type alias name.")?.clone();
        self.consume(TokenType::Equal, "Expect '=' after type alias name.")?;
        let target = self.parse_type()?;
        self.consume(TokenType::Semicolon, "Expect ';' after type alias.")?;
        Ok(Stmt::TypeAlias { name, target })
    }

    fn struct_declaration(&mut self) -> Result<Stmt, String> {
        let name = self.consume(TokenType::Identifier, "Expect struct name.")?.clone();
        self.consume(TokenType::LeftBrace, "Expect '{' before struct body.")?;
        let mut fields = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            let field_name = self.consume(TokenType::Identifier, "Expect field name.")?.clone();
            let mut field_type = None;
            if self.match_token(&[TokenType::Colon]) {
                field_type = Some(self.parse_type()?);
            }
            fields.push((field_name, field_type));
            // Optional comma or newline separation. Let's not strictly require comma in Quin struct definition, just identifiers.
            self.match_token(&[TokenType::Comma]);
        }
        self.consume(TokenType::RightBrace, "Expect '}' after struct body.")?;
        Ok(Stmt::Struct { name, fields })
    }

    fn class_declaration(&mut self, is_abstract: bool) -> Result<Stmt, String> {
        let name = self.consume(TokenType::Identifier, "Expect class name.")?.clone();
        
        let superclass = if self.match_token(&[TokenType::Extends]) {
            Some(self.consume(TokenType::Identifier, "Expect superclass name.")?.clone())
        } else {
            None
        };

        let mut interfaces = Vec::new();
        if self.match_token(&[TokenType::With]) {
            loop {
                interfaces.push(self.consume(TokenType::Identifier, "Expect trait name.")?.clone());
                if !self.match_token(&[TokenType::Comma]) { break; }
            }
        }

        self.consume(TokenType::LeftBrace, "Expect '{' before class body.")?;
        
        let mut methods = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            let mut vis = Visibility::Private; // Default to private for class members
            if self.match_token(&[TokenType::Pub]) { vis = Visibility::Public; }
            else if self.match_token(&[TokenType::Priv]) { vis = Visibility::Private; }

            let is_static = self.match_token(&[TokenType::Shared]);
            let is_async = self.match_token(&[TokenType::Async]);
            let is_abs = self.match_token(&[TokenType::Base]);

            if self.match_token(&[TokenType::Task]) || is_async {
                methods.push(self.task_declaration(vis, is_static, is_async, is_abs)?);
            } else if self.check(TokenType::Identifier) && self.peek().lexeme == "init" {
                self.advance();
                self.consume(TokenType::LeftParen, "Expect '(' after init.")?;
                let params = self.parse_params()?;
                self.consume(TokenType::RightParen, "Expect ')' after parameters.")?;
                self.consume(TokenType::LeftBrace, "Expect '{' before init body.")?;
                let body = self.block()?;
                methods.push(Stmt::Function {
                    name: Token { ty: TokenType::Identifier, lexeme: "init".to_string(), line: 0, column: 0 },
                    params, variadic: false, is_async: false, is_static: false, is_abstract: false,
                    visibility: Visibility::Public, return_type: None, body
                });
            } else if self.match_token(&[TokenType::Let]) || self.check(TokenType::Identifier) {
                // Property
                self.match_token(&[TokenType::Mut]); // consume mut if present
                let _name = self.consume(TokenType::Identifier, "Expect property name.")?;
                if self.match_token(&[TokenType::Colon]) {
                    self.parse_type()?;
                }
                self.consume(TokenType::Semicolon, "Expect ';' after property.")?;
            } else {
                return Err("Expect method or property in class.".to_string());
            }
        }
        self.consume(TokenType::RightBrace, "Expect '}' after class body.")?;

        Ok(Stmt::Class { name, superclass, is_abstract, interfaces, methods })
    }

    fn import_declaration(&mut self) -> Result<Stmt, String> {
        if self.match_token(&[TokenType::LeftBrace]) {
            // import { a, b } from math
            let mut items = Vec::new();
            if !self.check(TokenType::RightBrace) {
                loop {
                    items.push(self.consume(TokenType::Identifier, "Expect item name.")?.clone());
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
            }
            self.consume(TokenType::RightBrace, "Expect '}' after destructured import.")?;
            self.consume(TokenType::From, "Expect 'from' after destructured import.")?;
            let module = if self.match_token(&[TokenType::Identifier, TokenType::StringLit]) {
                let mut token = self.previous().clone();
                if token.ty == TokenType::StringLit {
                    token.lexeme.remove(0);
                    token.lexeme.pop();
                }
                token
            } else {
                return Err("Expect module name or path.".to_string());
            };
            self.consume(TokenType::Semicolon, "Expect ';' after import.")?;
            Ok(Stmt::Import { module, items })
        } else {
            let module = if self.match_token(&[TokenType::Identifier, TokenType::StringLit]) {
                let mut token = self.previous().clone();
                if token.ty == TokenType::StringLit {
                    token.lexeme.remove(0);
                    token.lexeme.pop();
                }
                token
            } else {
                return Err("Expect module name or path.".to_string());
            };
            self.consume(TokenType::Semicolon, "Expect ';' after import.")?;
            Ok(Stmt::Import { module, items: vec![] })
        }
    }

    fn interface_declaration(&mut self) -> Result<Stmt, String> {
        let name = self.consume(TokenType::Identifier, "Expect trait name.")?.clone();
        self.consume(TokenType::LeftBrace, "Expect '{' before trait body.")?;
        let mut methods = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            let is_async = self.match_token(&[TokenType::Async]);
            self.consume(TokenType::Task, "Expect 'task' in trait method signature.")?;
            methods.push(self.task_declaration(Visibility::Public, false, is_async, true)?);
        }
        self.consume(TokenType::RightBrace, "Expect '}' after trait body.")?;
        Ok(Stmt::Interface { name, methods })
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        if self.match_token(&[TokenType::If]) { return self.if_statement(); }
        if self.match_token(&[TokenType::For]) { return self.for_statement(); }
        if self.match_token(&[TokenType::While]) { return self.while_statement(); }
        if self.match_token(&[TokenType::Attempt]) { return self.try_statement(); }
        if self.match_token(&[TokenType::Match]) { return self.match_statement(); }
        if self.match_token(&[TokenType::Return]) { return self.return_statement(); }
        if self.match_token(&[TokenType::LeftBrace]) { return Ok(Stmt::Block(self.block()?)); }
        self.expression_statement()
    }

    fn if_statement(&mut self) -> Result<Stmt, String> {
        let condition = self.expression()?;
        self.consume(TokenType::LeftBrace, "Expect '{' after if condition.")?;
        let then_branch = Box::new(Stmt::Block(self.block()?));
        
        let mut elif_branches = Vec::new();
        while self.match_token(&[TokenType::Elif]) {
            let elif_cond = self.expression()?;
            self.consume(TokenType::LeftBrace, "Expect '{' after elif condition.")?;
            let elif_body = Stmt::Block(self.block()?);
            elif_branches.push((elif_cond, elif_body));
        }

        let mut else_branch = None;
        if self.match_token(&[TokenType::Else]) {
            self.consume(TokenType::LeftBrace, "Expect '{' after else.")?;
            else_branch = Some(Box::new(Stmt::Block(self.block()?)));
        }

        Ok(Stmt::If { condition, then_branch, elif_branches, else_branch })
    }

    fn for_statement(&mut self) -> Result<Stmt, String> {
        let item = self.consume(TokenType::Identifier, "Expect loop variable.")?.clone();
        self.consume(TokenType::In, "Expect 'in' after loop variable.")?;
        let iterable = self.expression()?;
        self.consume(TokenType::LeftBrace, "Expect '{' after loop iterable.")?;
        let body = Box::new(Stmt::Block(self.block()?));
        Ok(Stmt::For { item, iterable, body })
    }

    fn while_statement(&mut self) -> Result<Stmt, String> {
        let condition = self.expression()?;
        self.consume(TokenType::LeftBrace, "Expect '{' after while condition.")?;
        let body = Box::new(Stmt::Block(self.block()?));
        Ok(Stmt::While { condition, body })
    }

    fn match_statement(&mut self) -> Result<Stmt, String> {
        let expression = self.expression()?;
        self.consume(TokenType::LeftBrace, "Expect '{' after match expression.")?;
        let mut arms = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            if self.match_token(&[TokenType::Identifier]) && self.previous().lexeme == "_" {
                self.consume(TokenType::FatArrow, "Expect '=>' after '_'.")?;
                let stmt = self.statement()?;
                arms.push(MatchArm::Default(stmt));
            } else {
                let expr1 = self.expression()?;
                if self.match_token(&[TokenType::DotDot]) {
                    // It's a range
                    let min = match expr1 {
                        Expr::Literal(Literal::Int(i)) => i,
                        _ => return Err("Range starts must be integers.".to_string())
                    };
                    
                    let mut max = std::i64::MAX;
                    if !self.check(TokenType::FatArrow) {
                        let expr2 = self.expression()?;
                        max = match expr2 {
                            Expr::Literal(Literal::Int(i)) => i,
                            _ => return Err("Range ends must be integers.".to_string())
                        };
                    }

                    self.consume(TokenType::FatArrow, "Expect '=>' after range.")?;
                    let stmt = self.statement()?;
                    arms.push(MatchArm::Range(min, max, stmt));
                } else {
                    self.consume(TokenType::FatArrow, "Expect '=>' after match value.")?;
                    let stmt = self.statement()?;
                    arms.push(MatchArm::Value(expr1, stmt));
                }
            }
        }
        self.consume(TokenType::RightBrace, "Expect '}' after match arms.")?;
        Ok(Stmt::Match { expression, arms })
    }

    fn try_statement(&mut self) -> Result<Stmt, String> {
        self.consume(TokenType::LeftBrace, "Expect '{' after attempt.")?;
        let try_body = Box::new(Stmt::Block(self.block()?));
        self.consume(TokenType::Rescue, "Expect 'rescue' after attempt block.")?;
        self.consume(TokenType::LeftParen, "Expect '(' after rescue.")?;
        let catch_param = self.consume(TokenType::Identifier, "Expect error variable name.")?.clone();
        
        if self.match_token(&[TokenType::Colon]) {
            self.parse_type()?;
        }

        self.consume(TokenType::RightParen, "Expect ')' after error variable.")?;
        self.consume(TokenType::LeftBrace, "Expect '{' after rescue parens.")?;
        let catch_body = Box::new(Stmt::Block(self.block()?));
        
        Ok(Stmt::TryCatch { try_body, catch_param, catch_body })
    }

    fn throw_statement(&mut self) -> Result<Stmt, String> {
        let value = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after raise.")?;
        Ok(Stmt::Throw(value))
    }

    fn return_statement(&mut self) -> Result<Stmt, String> {
        let keyword = self.previous().clone();
        // Since we don't have semicolons rigidly, we just look ahead. If it's a newline (which we ignore usually) 
        // we might issue false positives, but let's assume if `{` or `}` block ends, it's done. 
        // Simple heuristic: expression unless right brace or end.
        let value = if !self.check(TokenType::RightBrace) && !self.is_at_end() && !self.check(TokenType::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };
        self.consume(TokenType::Semicolon, "Expect ';' after return value.")?;
        Ok(Stmt::Return { keyword, value })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        let mut statements = Vec::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.")?;
        Ok(statements)
    }

    fn expression_statement(&mut self) -> Result<Stmt, String> {
        let expr = self.expression()?;
        if !self.check(TokenType::RightBrace) {
            self.consume(TokenType::Semicolon, "Expect ';' after expression.")?;
        } else {
            self.match_token(&[TokenType::Semicolon]); // Optional
        }
        Ok(Stmt::Expression(expr))
    }

    pub fn expression(&mut self) -> Result<Expr, String> {
        self.pipe()
    }

    fn pipe(&mut self) -> Result<Expr, String> {
        let mut expr = self.assignment()?;
        while self.match_token(&[TokenType::PipeGreater]) {
            let right = self.assignment()?;
            expr = Expr::Pipe { left: Box::new(expr), right: Box::new(right) };
        }
        Ok(expr)
    }

    fn assignment(&mut self) -> Result<Expr, String> {
        let expr = self.nullish()?;

        if self.match_token(&[TokenType::Equal, TokenType::PlusEqual, TokenType::MinusEqual]) {
            let operator = self.previous().clone();
            let mut value = self.assignment()?;

            if operator.ty == TokenType::PlusEqual || operator.ty == TokenType::MinusEqual {
                let binary_op = if operator.ty == TokenType::PlusEqual {
                    Token::new(TokenType::Plus, "+".to_string(), operator.line, operator.column)
                } else {
                    Token::new(TokenType::Minus, "-".to_string(), operator.line, operator.column)
                };
                value = Expr::Binary {
                    left: Box::new(expr.clone()),
                    operator: binary_op,
                    right: Box::new(value),
                };
            }

            match expr {
                Expr::Variable(name) => {
                    return Ok(Expr::Assign { name, value: Box::new(value) });
                }
                Expr::Get { object, name } => {
                    return Ok(Expr::Set { object, name, value: Box::new(value) });
                }
                Expr::Index { object, bracket, index } => {
                    return Ok(Expr::IndexSet { object, bracket, index, value: Box::new(value) });
                }
                _ => return Err("Invalid assignment target.".to_string())
            }
        }
        Ok(expr)
    }

    fn nullish(&mut self) -> Result<Expr, String> {
        let mut expr = self.equality()?;
        while self.match_token(&[TokenType::Nullish]) {
            let operator = self.previous().clone();
            let right = self.equality()?;
            expr = Expr::Logical { left: Box::new(expr), operator, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.comparison()?;
        while self.match_token(&[TokenType::EqualEqual, TokenType::BangEqual]) {
            let operator = self.previous().clone();
            let right = self.comparison()?;
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.term()?;
        while self.match_token(&[TokenType::Greater, TokenType::GreaterEqual, TokenType::Less, TokenType::LessEqual]) {
            let operator = self.previous().clone();
            let right = self.term()?;
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut expr = self.factor()?;
        while self.match_token(&[TokenType::Minus, TokenType::Plus]) {
            let operator = self.previous().clone();
            let right = self.factor()?;
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.unary()?;
        while self.match_token(&[TokenType::Slash, TokenType::Star]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.match_token(&[TokenType::Bang, TokenType::Minus]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            return Ok(Expr::Unary { operator, right: Box::new(right) });
        }
        self.call()
    }

    fn call(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;

        loop {
            if self.match_token(&[TokenType::LeftParen]) {
                expr = self.finish_call(expr)?;
            } else if self.match_token(&[TokenType::Dot]) {
                let name = self.consume(TokenType::Identifier, "Expect property name after '.'.")?.clone();
                expr = Expr::Get { object: Box::new(expr), name };
            } else if self.match_token(&[TokenType::QuestionDot]) {
                let name = self.consume(TokenType::Identifier, "Expect property name after '?.' .")?.clone();
                expr = Expr::OptionalGet { object: Box::new(expr), name };
            } else if self.match_token(&[TokenType::LeftBracket]) {
                let bracket = self.previous().clone();
                let index = self.expression()?;
                self.consume(TokenType::RightBracket, "Expect ']' after index.")?;
                expr = Expr::Index { object: Box::new(expr), bracket, index: Box::new(index) };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn finish_call(&mut self, callee: Expr) -> Result<Expr, String> {
        let mut arguments = Vec::new();
        if !self.check(TokenType::RightParen) {
            loop {
                arguments.push(self.expression()?);
                if !self.match_token(&[TokenType::Comma]) { break; }
            }
        }
        let paren = self.consume(TokenType::RightParen, "Expect ')' after arguments.")?.clone();
        Ok(Expr::Call { callee: Box::new(callee), paren, arguments })
    }

    fn extract_last_expr(&self, stmts: Vec<Stmt>) -> Expr {
        if let Some(Stmt::Expression(expr)) = stmts.last() {
            expr.clone()
        } else {
            Expr::Literal(Literal::Null)
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        if self.match_token(&[TokenType::False]) { return Ok(Expr::Literal(Literal::Bool(false))); }
        if self.match_token(&[TokenType::True]) { return Ok(Expr::Literal(Literal::Bool(true))); }
        if self.match_token(&[TokenType::Void]) { return Ok(Expr::Literal(Literal::Null)); }

        if self.match_token(&[TokenType::IntLit]) {
            let i = self.previous().lexeme.parse::<i64>().unwrap_or(0);
            return Ok(Expr::Literal(Literal::Int(i)));
        }
        if self.match_token(&[TokenType::FloatLit]) {
            let f = self.previous().lexeme.parse::<f64>().unwrap_or(0.0);
            return Ok(Expr::Literal(Literal::Float(f)));
        }
        if self.match_token(&[TokenType::StringLit]) {
            let token = self.previous().clone();
            let mut s = token.lexeme.clone();
            s.remove(0); s.pop(); // strip quotes
            
            if !s.contains('{') {
                return Ok(Expr::Literal(Literal::String(s)));
            }
            
            let mut parts = Vec::new();
            let mut current_str = String::new();
            let mut chars = s.chars().peekable();
            
            while let Some(c) = chars.next() {
                if c == '{' {
                    if !current_str.is_empty() {
                        parts.push(Expr::Literal(Literal::String(current_str.clone())));
                        current_str.clear();
                    }
                    let mut expr_str = String::new();
                    while let Some(&inner_c) = chars.peek() {
                        if inner_c == '}' {
                            chars.next(); // consume '}'
                            break;
                        }
                        expr_str.push(inner_c);
                        chars.next();
                    }
                    if expr_str.is_empty() { continue; }
                    
                    // Parse inner expression
                    let mut lexer = crate::lexer::Lexer::new(&expr_str);
                    let mut tokens = lexer.scan_tokens()?;
                    if tokens.last().map_or(false, |t| t.ty == TokenType::Eof) {
                        tokens.pop();
                    }
                    let mut parser = Parser::new(tokens);
                    let expr = parser.expression()?;
                    parts.push(expr);
                } else {
                    current_str.push(c);
                }
            }
            if !current_str.is_empty() {
                parts.push(Expr::Literal(Literal::String(current_str)));
            }

            if parts.is_empty() {
                return Ok(Expr::Literal(Literal::String("".to_string())));
            }

            let mut final_expr = parts[0].clone();
            let plus_token = Token::new(TokenType::Plus, "+".to_string(), token.line, token.column);
            
            for i in 1..parts.len() {
                final_expr = Expr::Binary {
                    left: Box::new(final_expr),
                    operator: plus_token.clone(),
                    right: Box::new(parts[i].clone())
                };
            }

            return Ok(final_expr);
        }

        if self.match_token(&[TokenType::Identifier]) {
            let name = self.previous().clone();
            if self.check(TokenType::LeftBrace) && self.is_struct_init_start() {
                self.match_token(&[TokenType::LeftBrace]);
                // Struct Init handling e.g. Person { name: "x" }
                let mut fields = Vec::new();
                if !self.check(TokenType::RightBrace) {
                    loop {
                        let field_name = self.consume(TokenType::Identifier, "Expect field name.")?.clone();
                        self.consume(TokenType::Colon, "Expect ':' after field name.")?;
                        let val = self.expression()?;
                        fields.push((field_name, val));
                        if !self.match_token(&[TokenType::Comma]) { break; }
                    }
                }
                self.consume(TokenType::RightBrace, "Expect '}' after struct fields.")?;
                return Ok(Expr::StructInit { name, fields });
            }
            return Ok(Expr::Variable(name));
        }

        if self.match_token(&[TokenType::LeftBrace]) {
            // Dict or Set. Check for ':' to distinguish.
            // Simplified: if empty, it's a Dict (like JS). 
            // If it has 'key: val', it's a Dict. 
            // If it has 'val, val', it's a Set (maybe?). 
            // User requested `set`, so maybe `set { ... }` is better.
            let mut entries = Vec::new();
            if !self.check(TokenType::RightBrace) {
                loop {
                    let key = self.expression()?;
                    if self.match_token(&[TokenType::Colon]) {
                        let value = self.expression()?;
                        entries.push((key, value));
                    } else {
                        // It's a set? Or just a single-entry dict? 
                        // Let's assume dict for now if we use this syntax.
                        return Err("Expect ':' after dict key.".to_string());
                    }
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
            }
            self.consume(TokenType::RightBrace, "Expect '}' after dict.")?;
            return Ok(Expr::Dict { entries });
        }

        if self.match_token(&[TokenType::Set]) {
            self.consume(TokenType::LeftBrace, "Expect '{' after 'set'.")?;
            let mut elements = Vec::new();
            if !self.check(TokenType::RightBrace) {
                loop {
                    elements.push(self.expression()?);
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
            }
            self.consume(TokenType::RightBrace, "Expect '}' after set.")?;
            return Ok(Expr::SetInit { elements });
        }

        if self.match_token(&[TokenType::LeftBracket]) {
            let mut elements = Vec::new();
            if !self.check(TokenType::RightBracket) {
                loop {
                    if self.match_token(&[TokenType::DotDotDot]) {
                        elements.push(Expr::Spread(Box::new(self.expression()?)));
                    } else {
                        elements.push(self.expression()?);
                    }
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
            }
            self.consume(TokenType::RightBracket, "Expect ']' after array elements.")?;
            return Ok(Expr::Array { elements });
        }

        if self.match_token(&[TokenType::LeftParen]) {
            if self.check(TokenType::RightParen) {
                self.advance();
                return Ok(Expr::Tuple { elements: vec![] });
            }
            let first = self.expression()?;
            if self.match_token(&[TokenType::Comma]) {
                let mut elements = vec![first];
                while !self.check(TokenType::RightParen) && !self.is_at_end() {
                    elements.push(self.expression()?);
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
                self.consume(TokenType::RightParen, "Expect ')' after tuple elements.")?;
                return Ok(Expr::Tuple { elements });
            }
            self.consume(TokenType::RightParen, "Expect ')' after expression.")?;
            return Ok(first);
        }

        if self.match_token(&[TokenType::If]) {
            // To avoid ambiguity with struct literals: if condition { ... }
            // we can temporarily disable struct literals OR expect the condition to be an expression
            // that doesn't end with a struct literal.
            // Simplified approach: check if we have '(' immediately.
            let condition = self.expression()?;
            self.consume(TokenType::LeftBrace, "Expect '{' after if condition.")?;
            let then_block = self.block()?;
            self.consume(TokenType::Else, "Expect 'else' for inline if.")?;
            self.consume(TokenType::LeftBrace, "Expect '{' after else.")?;
            let else_block = self.block()?;
            
            return Ok(Expr::Ternary { 
                 condition: Box::new(condition),
                 then_branch: Box::new(self.extract_last_expr(then_block)),
                 else_branch: Box::new(self.extract_last_expr(else_block)),
            });
        }

        if self.match_token(&[TokenType::Task]) {
            // Lambda: task(params) { body } or task(params) => expr
            self.consume(TokenType::LeftParen, "Expect '(' after 'task' in lambda.")?;
            let mut params = Vec::new();
            if !self.check(TokenType::RightParen) {
                loop {
                    let name = self.consume(TokenType::Identifier, "Expect parameter name.")?.clone();
                    let param_type = if self.match_token(&[TokenType::Colon]) { Some(self.parse_type()?) } else { None };
                    params.push((name, param_type, None));
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
            }
            self.consume(TokenType::RightParen, "Expect ')' after parameters.")?;
            
            // Optional return type
            let _return_type = if self.match_token(&[TokenType::Arrow]) {
                Some(self.parse_type()?)
            } else {
                None
            };
            
            let body = if self.match_token(&[TokenType::FatArrow]) {
                let expr = self.expression()?;
                let return_kw = Token::new(TokenType::Return, "return".to_string(), self.previous().line, self.previous().column);
                vec![Stmt::Return { keyword: return_kw, value: Some(expr) }]
            } else {
                self.consume(TokenType::LeftBrace, "Expect '{' before lambda body.")?;
                self.block()?
            };

            return Ok(Expr::Lambda { 
                params,
                body,
                is_async: false,
            });
        }

        if self.match_token(&[TokenType::Self_]) {
            return Ok(Expr::Variable(self.previous().clone()));
        }

        Err(format!("Expect expression, found {:?}", self.peek().ty))
    }

    fn match_token(&mut self, types: &[TokenType]) -> bool {
        for ty in types {
            if self.check(ty.clone()) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check(&self, ty: TokenType) -> bool {
        if self.is_at_end() { return false; }
        self.peek().ty == ty
    }



    fn is_struct_init_start(&self) -> bool {
        // Look ahead for '{' followed by 'identifier :'
        if !self.check(TokenType::LeftBrace) { return false; }
        if self.current + 2 >= self.tokens.len() { return false; }
        self.tokens[self.current + 1].ty == TokenType::Identifier && 
        self.tokens[self.current + 2].ty == TokenType::Colon
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() { self.current += 1; }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().ty == TokenType::Eof
    }

    fn peek(&self) -> &Token {
        if self.current >= self.tokens.len() {
            return self.tokens.last().unwrap();
        }
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn consume(&mut self, ty: TokenType, message: &str) -> Result<&Token, String> {
        if self.check(ty) {
            Ok(self.advance())
        } else {
            Err(format!("{} (at line {}, lexeme: '{}')", message, self.peek().line, self.peek().lexeme))
        }
    }


    fn parse_type(&mut self) -> Result<crate::ast::Type, String> {
        let mut ty = if self.match_token(&[TokenType::Any]) {
            crate::ast::Type::Any
        } else if self.match_token(&[TokenType::Dict]) {
            self.consume(TokenType::Less, "Expect '<' after 'dict'.")?;
            let key = self.parse_type()?;
            self.consume(TokenType::Comma, "Expect ',' between key and value types.")?;
            let value = self.parse_type()?;
            self.consume(TokenType::Greater, "Expect '>' after dict types.")?;
            crate::ast::Type::Dict(Box::new(key), Box::new(value))
        } else if self.match_token(&[TokenType::Set]) {
            self.consume(TokenType::Less, "Expect '<' after 'set'.")?;
            let inner = self.parse_type()?;
            self.consume(TokenType::Greater, "Expect '>' after set type.")?;
            crate::ast::Type::Set(Box::new(inner))
        } else if self.match_token(&[TokenType::Tuple]) {
            self.consume(TokenType::Less, "Expect '<' after 'tuple'.")?;
            let mut types = Vec::new();
            if !self.check(TokenType::Greater) {
                loop {
                    types.push(self.parse_type()?);
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
            }
            self.consume(TokenType::Greater, "Expect '>' after tuple types.")?;
            crate::ast::Type::Tuple(types)
        } else if self.match_token(&[TokenType::LeftParen]) {
            let mut types = Vec::new();
            if !self.check(TokenType::RightParen) {
                loop {
                    types.push(self.parse_type()?);
                    if !self.match_token(&[TokenType::Comma]) { break; }
                }
            }
            self.consume(TokenType::RightParen, "Expect ')' after tuple types.")?;
            crate::ast::Type::Tuple(types)
        } else if self.match_token(&[
            TokenType::Identifier,
            TokenType::TypeInt,
            TokenType::TypeStr,
            TokenType::TypeFloat,
            TokenType::TypeBool,
            TokenType::Void,
        ]) {
            crate::ast::Type::Simple(self.previous().lexeme.clone())
        } else {
            return Err(format!("Expect type at line {}", self.peek().line));
        };

        // Handle array brackets
        while self.match_token(&[TokenType::LeftBracket]) {
            self.consume(TokenType::RightBracket, "Expect ']' after '['.")?;
            ty = crate::ast::Type::Array(Box::new(ty));
        }

        // Handle Union types
        if self.match_token(&[TokenType::Pipe]) {
            let mut types = vec![ty];
            loop {
                types.push(self.parse_type()?);
                if !self.match_token(&[TokenType::Pipe]) { break; }
            }
            ty = crate::ast::Type::Union(types);
        }

        Ok(ty)
    }
}
