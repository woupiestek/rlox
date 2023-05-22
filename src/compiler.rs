use std::mem;

use crate::{
    chunk::Op,
    memory::Heap,
    object::{Class, Function, Value},
    scanner::{Scanner, Token, TokenType},
};

pub enum Precedence {
    None,
    Assignment, // =
    Or,         // or
    And,        // and
    Equality,   // == !=
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * /
    Unary,      // ! -
    Call,       // . ()
    Primary,
}
struct Local {
    name: String,
    depth: i32,
    is_captured: bool,
}

#[derive(Clone, Copy)]
struct Upvalue {
    index: u8,
    is_local: bool,
}

#[derive(Eq, PartialEq)]
enum FunctionType {
    Function,
    Initializer,
    Method,
    Script,
}

struct Compiler {
    function: Function,
    upvalues: Vec<Upvalue>,
    function_type: FunctionType,
    scope_depth: i32,
    locals: Vec<Local>,
}

impl Compiler {
    fn new() -> Self {
        Self {
            function: Function::new(),
            upvalues: Vec::new(),
            function_type: FunctionType::Script,
            scope_depth: 0,
            locals: Vec::new(),
        }
    }

    fn resolve_local(&self, name: &str) -> Result<Option<u8>, String> {
        let mut i = self.locals.len() - 1;
        loop {
            let local = &self.locals[i];
            if local.name == name {
                return if local.depth == -1 {
                    Err("Can't read local variable in its own initializer.".to_string())
                } else {
                    Ok(Some(i as u8))
                };
            }
            if i == 0 {
                return Ok(None); // same as -1 ?
            } else {
                i -= 1;
                continue;
            }
        }
    }

    fn add_local(&mut self, name: &str) -> Result<(), String> {
        if self.locals.len() == 256 {
            return Err("Too many local variables in function.".to_string());
        }
        self.locals.push(Local {
            name: name.to_string(),
            depth: -1,
            is_captured: false,
        });
        return Ok(());
    }

    fn mark_initialized(&mut self) -> bool {
        if self.scope_depth == 0 {
            return false;
        }
        let i = self.locals.len() - 1;
        self.locals[i].depth = self.scope_depth;
        return true;
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<u8, String> {
        let len = self.upvalues.len();
        for i in 0..len {
            let upvalue = &self.upvalues[i];
            if upvalue.is_local == is_local && upvalue.index == index {
                return Ok(i as u8);
            }
        }
        if len > u8::MAX as usize {
            return Err("Too many closure variables in function.".to_string());
        }
        self.upvalues.push(Upvalue { index, is_local });
        Ok(len as u8)
    }

    fn count(&mut self) -> usize {
        self.function.chunk.count()
    }

    fn make_constant(&mut self, value: Value) -> Result<u8, String> {
        self.function.chunk.add_constant(value)
    }

    fn patch_jump(&mut self, offset: usize) -> Result<(), String> {
        let jump = self.count() - offset - 2;
        if jump > u16::MAX as usize {
            Err("jump too large".to_string())
        } else {
            self.function
                .chunk
                .over_write(&[(jump >> 8) as u8, jump as u8], offset);
            Ok(())
        }
    }

    fn declare_variable(&mut self, name: &str) -> Result<(), String> {
        if self.scope_depth == 0 {
            return Ok(());
        }
        for local in &self.locals {
            if local.name == name {
                return Err("Already a variable with this name in this scope.".to_string());
            }
        }
        self.add_local(name)
    }
}

pub struct Source<'b> {
    source: &'b str,
    scanner: Scanner<'b>,
    current_token: Token,
    previous_token: Token,
}

impl Source<'_> {
    pub fn new(source: &str) -> Source {
        let mut scanner = Scanner::new(source);
        let current = scanner.next();
        Source {
            source,
            scanner,
            current_token: current,
            previous_token: Token::nil(),
        }
    }

    // I guess this simplification means that:
    //
    // 1 error tokens are not always reported
    // 2 sometimes an error token makes its way up to the parser and
    // causes failure there.
    //
    // report and continue, maybe that should be the pattern.
    fn advance(&mut self) {
        self.previous_token = self.current_token;
        self.current_token = self.scanner.next();
    }

    fn check(&mut self, token_type: TokenType) -> bool {
        self.current_token.token_type == token_type
    }

    fn match_type(&mut self, token_type: TokenType) -> bool {
        if self.check(token_type) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume<'b>(&mut self, token_type: TokenType, msg: &'b str) -> Result<(), &'b str> {
        if self.check(token_type) {
            Ok(self.advance())
        } else {
            Err(msg)
        }
    }

    fn line(&self) -> u16 {
        self.previous_token.line
    }

    fn synchronize(&mut self) -> Result<(), String> {
        loop {
            match self.current_token.token_type {
                TokenType::Class
                | TokenType::End
                | TokenType::Fun
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => {
                    return Ok(());
                }
                TokenType::Semicolon => {
                    self.advance();
                    return Ok(());
                }
                _ => {
                    self.advance();
                    continue;
                }
            }
        }
    }
}

pub struct Parser<'b> {
    // source
    source: Source<'b>,

    // targets
    current_compiler: Compiler,
    compilers: Vec<Compiler>,

    has_super: bool,
    had_super: Vec<bool>,
    // helper service
    heap: Heap,
}

// why couldn't this be a method!?
macro_rules! lexeme {
    ($s:ident) => {
        &$s.source.source[$s.source.previous_token.from..$s.source.previous_token.to]
    };
}

impl Parser<'_> {
    pub fn new(source: Source, heap: Heap) -> Parser {
        Parser {
            source,
            current_compiler: Compiler::new(),
            compilers: Vec::new(),
            has_super: false,
            had_super: Vec::new(),
            heap,
        }
    }

    fn emit_bytes(&mut self, bytes: &[u8]) {
        self.current_compiler
            .function
            .chunk
            .write(bytes, self.source.line());
    }

    fn emit_loop(&mut self, start: usize) -> Result<(), String> {
        let offset = self.current_compiler.count() - start + 2;
        if offset > u16::MAX as usize {
            Err("loop size to large".to_string())
        } else {
            self.emit_bytes(&[Op::Loop as u8, (offset >> 8) as u8, offset as u8]);
            Ok(())
        }
    }

    fn emit_jump(&mut self, instruction: u8) -> usize {
        self.emit_bytes(&[instruction, 0xff, 0xff]);
        self.current_compiler.count() - 2
    }

    fn emit_return(&mut self) {
        if self.current_compiler.function_type == FunctionType::Initializer {
            self.emit_bytes(&[Op::GetLocal as u8, 0, Op::Return as u8]);
        } else {
            self.emit_bytes(&[Op::Nil as u8, Op::Return as u8]);
        }
    }

    fn emit_constant(&mut self, value: Value) -> Result<(), String> {
        let make_constant = self.current_compiler.make_constant(value)?;
        Ok(self.emit_bytes(&[Op::Constant as u8, make_constant]))
    }

    fn begin_scope(&mut self) {
        self.current_compiler.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.current_compiler.scope_depth -= 1;
        while let Some(local) = self.current_compiler.locals.last() {
            if local.depth <= self.current_compiler.scope_depth {
                break;
            }
            self.emit_bytes(&[if local.is_captured {
                Op::CloseUpvalue
            } else {
                Op::Pop
            } as u8]);
        }
    }

    fn string_value(&mut self, str: &str) -> Value {
        Value::Obj(self.heap.store(str.to_string()).downgrade())
    }

    fn resolve_upvalue(&mut self, name: &str) -> Result<Option<u8>, String> {
        if self.compilers.len() == 0 {
            return Ok(None);
        }
        let mut level = self.compilers.len() - 1;
        // find the local index
        let mut index = loop {
            if let Some(local) = self.compilers[level].resolve_local(name)? {
                self.compilers[level].locals[local as usize].is_captured = true;
                break local;
            }
            if level == 0 {
                return Ok(None);
            } else {
                level -= 1;
                continue;
            }
        };
        let mut is_local = true;
        // set upvalue indices...
        loop {
            level += 1;
            if level == self.compilers.len() {
                break;
            }
            index = self.compilers[level].add_upvalue(index as u8, is_local)?;
            is_local = false;
        }
        Ok(Some(
            self.current_compiler.add_upvalue(index as u8, is_local)?,
        ))
    }

    fn argument_list(&mut self) -> Result<u8, String> {
        if self.source.match_type(TokenType::RightParen) {
            return Ok(0);
        }
        let mut arity: u8 = 0;
        loop {
            self.expression()?;
            arity += 1;
            if self.source.match_type(TokenType::Comma) {
                if arity == u8::MAX {
                    return Err("Can't have more than 255 arguments.".to_string());
                }
                continue;
            } else {
                self.source
                    .consume(TokenType::RightParen, "Expect ')' after arguments.")?;
                return Ok(arity);
            }
        }
    }

    fn and(&mut self) -> Result<(), String> {
        let end_jump = self.emit_jump(Op::JumpIfFalse as u8);
        self.emit_bytes(&[Op::Pop as u8]);
        self.parse_precedence(Precedence::And)?;
        self.current_compiler.patch_jump(end_jump)
    }

    fn binary(&mut self) -> Result<(), String> {
        todo!()
    }

    fn call(&mut self) -> Result<(), String> {
        let arity = self.argument_list()?;
        Ok(self.emit_bytes(&[Op::Call as u8, arity]))
    }

    fn dot(&mut self, can_assign: bool) -> Result<(), String> {
        let index = self.identifier_constant("Expect property name after '.'.")?;
        if can_assign && self.source.match_type(TokenType::Equal) {
            self.expression()?;
            self.emit_bytes(&[Op::SetProperty as u8, index])
        } else if self.source.match_type(TokenType::LeftParen) {
            let arity = self.argument_list()?;
            self.emit_bytes(&[Op::Invoke as u8, index, arity]);
        } else {
            self.emit_bytes(&[Op::GetProperty as u8, index]);
        };
        Ok(())
    }

    fn intern(&mut self, name: &str) -> Result<u8, String> {
        let value = self.string_value(name);
        let index = self.current_compiler.make_constant(value)?;
        Ok(index)
    }

    fn identifier_constant(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        self.intern(lexeme!(self))
    }

    fn literal(&mut self, token_type: &TokenType) {
        self.emit_bytes(&[match token_type {
            TokenType::False => Op::False as u8,
            TokenType::Nil => Op::Nil as u8,
            TokenType::True => Op::True as u8,
            _ => panic!("'{}' mistaken for literal", lexeme!(self)),
        }]);
    }

    fn grouping(&mut self) -> Result<(), String> {
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after expression.")?;
        Ok(())
    }

    fn number(&mut self) -> Result<(), String> {
        match lexeme!(self).parse::<f64>() {
            Ok(number) => self.emit_constant(Value::Number(number)),
            Err(err) => Err(err.to_string()),
        }
    }

    // I want different logic...
    fn or(&mut self) -> Result<(), String> {
        //let end_jump = self.emit_jump(Op::JumpIfFalse as u8);
        //self.emit_bytes(&[Op::Pop as u8]);
        self.parse_precedence(Precedence::Or)?;
        //self.patch_jump(end_jump)
        todo!();
    }

    fn string(&mut self) -> Result<(), String> {
        let lexeme = lexeme!(self);
        let value = self.string_value(&lexeme[1..lexeme.len() - 1]);
        self.emit_constant(value)
    }

    // admit code for variable access
    fn variable(&mut self, name: &str, can_assign: bool) -> Result<(), String> {
        let (arg, get, set) = {
            if let Some(arg) = self.current_compiler.resolve_local(name)? {
                (arg, Op::GetGlobal as u8, Op::SetGlobal as u8)
            } else if let Some(arg) = self.resolve_upvalue(name)? {
                (arg, Op::GetUpvalue as u8, Op::SetUpvalue as u8)
            } else {
                let value = self.string_value(name);
                let arg = self.current_compiler.make_constant(value)?;
                (arg, Op::GetGlobal as u8, Op::SetGlobal as u8)
            }
        };

        if can_assign && self.source.match_type(TokenType::Equal) {
            self.expression()?;
            self.emit_bytes(&[set, arg]);
        } else {
            self.emit_bytes(&[get, arg]);
        }
        Ok(())
    }

    fn super_(&mut self) -> Result<(), String> {
        if self.compilers.is_empty() {
            return Err("Can't use 'super' outside of a class.".to_string());
        }
        if !self.has_super {
            return Err("Can't use 'super' in a class with no superclass.".to_string());
        }
        self.source
            .consume(TokenType::Dot, "Expect '.' after 'super'.")?;
        let index = self.identifier_constant("Expect superclass method name.")?;
        self.variable("this", false)?;
        if self.source.match_type(TokenType::LeftParen) {
            let arity = self.argument_list()?;
            self.variable("super", false)?;
            self.emit_bytes(&[Op::SuperInvoke as u8, index, arity]);
        } else {
            self.variable("super", false)?;
            self.emit_bytes(&[Op::GetSuper as u8, index]);
        }
        Ok(())
    }

    fn this(&mut self, can_assign: bool) -> Result<(), String> {
        if self.compilers.is_empty() {
            return Err("Can't use 'this' outside of a class.".to_string());
        }
        self.variable("this", can_assign)
    }

    fn unary(&mut self, token_type: TokenType) -> Result<(), String> {
        self.parse_precedence(Precedence::Unary)?;
        match token_type {
            TokenType::Bang => self.emit_bytes(&[Op::Not as u8]),
            TokenType::Minus => self.emit_bytes(&[Op::Negative as u8]),
            _ => panic!(),
        }
        Ok(())
    }

    fn parse_precedence(&mut self, precedence: Precedence) -> Result<(), String> {
        todo!()
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        let name = lexeme!(self);
        self.current_compiler.declare_variable(name)?;
        Ok(if self.current_compiler.scope_depth > 0 {
            0
        } else {
            let value = self.string_value(name);
            self.current_compiler.make_constant(value)?
        })
    }

    fn define_variable(&mut self, global: u8) {
        if !self.current_compiler.mark_initialized() {
            self.emit_bytes(&[Op::DefineGlobal as u8, global])
        }
    }

    fn expression(&mut self) -> Result<(), String> {
        self.parse_precedence(Precedence::Assignment)
    }

    fn block(&mut self) -> Result<(), String> {
        while !self.source.check(TokenType::RightBrace) && !self.source.check(TokenType::End) {
            self.declaration();
        }
        self.source
            .consume(TokenType::RightBrace, "Expect '}' after block.")?;
        Ok(())
    }

    fn function(&mut self, function_type: FunctionType, name: Option<u8>) -> Result<(), String> {
        assert_eq!(self.current_compiler.count(), 0, "assert empty chunk start");
        // grab the previous identifier for the name
        // may be strange for the top level script, but this is the way it is.
        // let name = lexeme!(self).to_string();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after function name.")?;
        let mut arity = 0u16;
        if !self.source.check(TokenType::RightParen) {
            loop {
                let index = self.parse_variable("Expect parameter name")?;
                self.define_variable(index);
                if self.source.match_type(TokenType::Comma) {
                    if arity < u8::MAX as u16 {
                        arity += 1;
                        continue;
                    } else {
                        return Err("Can't have more than 255 parameters.".to_string());
                    }
                }
                break;
            }
        }
        self.source
            .consume(TokenType::RightParen, "Expect ')' after parameters.")?;
        self.source
            .consume(TokenType::LeftBrace, "Expect '{' before function body")?;
        self.block()?;

        todo!();

        Ok(())
    }

    fn method(&mut self) -> Result<(), String> {
        self.source
            .consume(TokenType::Identifier, "Expect method name.")?;
        let name = lexeme!(self);
        let function_type = if name == "init" {
            FunctionType::Initializer
        } else {
            FunctionType::Method
        };
        let intern = self.intern(name)?;
        self.function(function_type, Some(intern))
        // emit what? why?
        // how do methods names get in scope?
        // ok, that are accessed as members, so no need?
    }

    //classDecl      → "class" IDENTIFIER ( "<" IDENTIFIER )? "{" function* "}" ;
    fn class(&mut self) -> Result<(), String> {
        // first thing in the class, so maybe not so spectacular?
        self.source
            .consume(TokenType::Identifier, "Expect class name.")?;
        let class_name = lexeme!(self);
        self.current_compiler.declare_variable(class_name)?;
        let index = self.intern(class_name)?;
        self.emit_bytes(&[Op::Class as u8, index]);
        self.define_variable(index);

        // start a new class; 184 byte replace!
        self.compilers
            .push(mem::replace(&mut self.current_compiler, Compiler::new()));
        // self.current_compiler.class.name = Some(index);

        // super decl
        if self.source.match_type(TokenType::Less) {
            self.source
                .consume(TokenType::Identifier, "Expect superclass name.")?;
            let super_name = lexeme!(self);
            self.variable(super_name, false)?;
            if class_name == super_name {
                return Err("A class can't inherit from itself.".to_string());
            }
            self.begin_scope();
            self.current_compiler.add_local("super")?;
            self.define_variable(0);
            self.variable(class_name, false)?;
            self.emit_bytes(&[Op::Inherit as u8]);
            self.has_super = true
        }

        // why this again?
        self.variable(class_name, false)?;

        // class body
        self.source
            .consume(TokenType::LeftBrace, "Expect '{' before class body.")?;
        loop {
            if self.source.match_type(TokenType::RightBrace) {
                break;
            }
            if self.source.check(TokenType::End) {
                return Err("Expect '}' after class body.".to_string());
            }
            self.method()?;
        }
        self.emit_bytes(&[Op::Pop as u8]);

        if self.has_super {
            self.end_scope();
        }

        // I can iterate over the upvalue, just not in a convenient way. Why not?
        let len = self.current_compiler.upvalues.len();
        for i in 0..len {
            let upvalue = self.current_compiler.upvalues[i];
            self.emit_bytes(&[if upvalue.is_local { 1 } else { 0 }, upvalue.index]);
        }
        // self.current_compiler.up_value_count = len as u8;

        todo!()
    }

    fn statement(&mut self) -> Result<(), String> {
        !todo!()
    }
    fn declaration(&mut self) {
        !todo!()
    }
}
