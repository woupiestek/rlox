use crate::{
    chunk::{Chunk, Op},
    memory::Heap,
    object::{Class, Method, Value},
    scanner::{Scanner, Token, TokenType},
};

struct Local {
    name: String,
    depth: i32,
    is_captured: bool,
}
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
    class: Class,
    has_super: bool,
    upvalues: Vec<Upvalue>,
    method: Method,
    function_type: FunctionType,
    scope_depth: i32,
    locals: Vec<Local>,
}

impl Compiler {
    fn new(function_type: FunctionType) -> Self {
        Self {
            class: Class::new(),
            has_super: false,
            upvalues: Vec::new(),
            method: Method::new(),
            function_type,
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

    fn mark_initialized(&mut self) {
        if self.scope_depth == 0 {
            return;
        }
        let i = self.locals.len() - 1;
        self.locals[i].depth = self.scope_depth
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
}

pub struct Parser<'b> {
    // source
    source: &'b str,
    scanner: Scanner<'b>,
    current_token: Token,

    // targets
    current_compiler: Compiler,
    compilers: Vec<Compiler>,

    // helper service
    heap: Heap,
}

impl Parser<'_> {
    pub fn new(source: &str, heap: Heap) -> Parser {
        let mut scanner = Scanner::new(source);
        let current = scanner.next();
        Parser {
            source,
            scanner,
            current_token: current,
            current_compiler: Compiler::new(FunctionType::Script),
            compilers: Vec::new(),
            heap,
        }
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.current_compiler.method.chunk
    }

    // I guess this simplification means that:
    //
    // 1 error tokens are not always reported
    // 2 sometimes an error token makes its way up to the parser and
    // causes failure there.
    //
    // report and continue, maybe that should be the pattern.
    fn advance(&mut self) -> Token {
        let current = self.current_token;
        self.current_token = self.scanner.next();
        current
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

    fn consume<'b>(&mut self, token_type: TokenType, msg: &'b str) -> Result<Token, &'b str> {
        if self.check(token_type) {
            Ok(self.advance())
        } else {
            Err(msg)
        }
    }

    fn emit_bytes(&mut self, bytes: &[u8]) {
        let line = self.current_token.line; // off by one?
        self.current_chunk().write(bytes, line)
    }

    fn emit_loop(&mut self, start: usize) -> Result<(), String> {
        let offset = self.current_chunk().count() - start + 2;
        if offset > u16::MAX as usize {
            Err("loop size to large".to_string())
        } else {
            self.emit_bytes(&[Op::Loop as u8, (offset >> 8) as u8, offset as u8]);
            Ok(())
        }
    }

    fn emit_jump(&mut self, instruction: u8) -> usize {
        self.emit_bytes(&[instruction, 0xff, 0xff]);
        return self.current_chunk().count() - 2;
    }

    fn emit_return(&mut self) {
        if self.current_compiler.function_type == FunctionType::Initializer {
            self.emit_bytes(&[Op::GetLocal as u8, 0, Op::Return as u8]);
        } else {
            self.emit_bytes(&[Op::Nil as u8, Op::Return as u8]);
        }
    }

    fn make_constant(&mut self, value: Value) -> Result<u8, String> {
        let constants = &mut self.current_compiler.class.constants;
        if constants.len() > u8::MAX as usize {
            return Err("too many constants in class".to_string());
        }
        let index = constants.len() as u8;
        constants.push(value);
        Ok(index)
    }

    fn emit_constant(&mut self, value: Value) -> Result<(), String> {
        let make_constant = self.make_constant(value)?;
        Ok(self.emit_bytes(&[Op::Constant as u8, make_constant]))
    }

    fn patch_jump(&mut self, offset: usize) -> Result<(), String> {
        let jump = self.current_chunk().count() - offset - 2;
        if jump > u16::MAX as usize {
            Err("jump too large".to_string())
        } else {
            self.current_chunk()
                .over_write(&[(jump >> 8) as u8, jump as u8], offset);
            Ok(())
        }
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

    fn identifier_constant(&mut self, name: &str) -> Result<u8, String> {
        let handle = self.heap.store(name.to_string()).downgrade();
        self.make_constant(Value::Obj(handle))
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

    fn declare_variable(&mut self, name: &str) -> Result<(), String> {
        if self.current_compiler.scope_depth == 0 {
            return Ok(());
        }
        for local in &self.current_compiler.locals {
            if local.name == name {
                return Err("Already a variable with this name in this scope.".to_string());
            }
        }
        self.current_compiler.add_local(name)
    }

    fn argument_list(&mut self) -> Result<u8, String> {
        if self.match_type(TokenType::RightParen) {
            return Ok(0);
        }
        let mut arity: u8 = 0;
        loop {
            self.expression()?;
            arity += 1;
            if self.match_type(TokenType::Comma) {
                if arity == u8::MAX {
                    return Err("Can't have more than 255 arguments.".to_string());
                }
                continue;
            } else {
                self.consume(TokenType::RightParen, "Expect ')' after arguments.")?;
                return Ok(arity);
            }
        }
    }

    fn and(&mut self) -> Result<(), String> {
        let end_jump = self.emit_jump(Op::JumpIfFalse as u8);
        self.emit_bytes(&[Op::Pop as u8]);
        self.parse_precedence(Precedence::And)?;
        self.patch_jump(end_jump)
    }

    fn binary(&mut self, previous_type: TokenType) -> Result<(), String> {
        todo!()
    }

    fn call(&mut self) -> Result<(), String> {
        let arity = self.argument_list()?;
        Ok(self.emit_bytes(&[Op::Call as u8, arity]))
    }

    fn dot(&mut self, can_assign: bool) -> Result<(), String> {
        let token = &self.consume(TokenType::Identifier, "Expect property name after '.'.")?;
        let name = self.identifier_constant(token.lexeme(self.source))?;
        if can_assign && self.match_type(TokenType::Equal) {
            self.expression()?;
            self.emit_bytes(&[Op::SetProperty as u8, name])
        } else if self.match_type(TokenType::LeftParen) {
            let arity = self.argument_list()?;
            self.emit_bytes(&[Op::Invoke as u8, name, arity]);
        } else {
            self.emit_bytes(&[Op::GetProperty as u8, name]);
        };
        Ok(())
    }

    fn literal(&mut self, token_type: &TokenType) {
        match token_type {
            TokenType::False => self.emit_bytes(&[Op::False as u8]),
            TokenType::Nil => self.emit_bytes(&[Op::Nil as u8]),
            TokenType::True => self.emit_bytes(&[Op::True as u8]),
            _ => panic!("is this allowed?"),
        }
    }

    fn grouping(&mut self) -> Result<(), String> {
        self.expression()?;
        self.consume(TokenType::RightParen, "Expect ')' after expression.")?;
        Ok(())
    }

    fn number(&mut self, previous: &Token) -> Result<(), String> {
        match previous.lexeme(self.source).parse::<f64>() {
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

    fn string(&mut self, previous: &Token) -> Result<(), String> {
        let lexeme = previous.lexeme(self.source);
        let handle = self
            .heap
            .store(lexeme[1..lexeme.len() - 1].to_string())
            .downgrade();
        self.emit_constant(Value::Obj(handle))
    }

    fn variable(&mut self, name: &str, can_assign: bool) -> Result<(), String> {
        let (arg, get, set) = {
            if let Some(arg) = self.current_compiler.resolve_local(name)? {
                (arg, Op::GetGlobal as u8, Op::SetGlobal as u8)
            } else if let Some(arg) = self.resolve_upvalue(name)? {
                (arg, Op::GetUpvalue as u8, Op::SetUpvalue as u8)
            } else {
                let arg = self.identifier_constant(name)?;
                (arg, Op::GetGlobal as u8, Op::SetGlobal as u8)
            }
        };

        if can_assign && self.match_type(TokenType::Equal) {
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
        if self.current_compiler.has_super {
            return Err("Can't use 'super' in a class with no superclass.".to_string());
        }
        self.consume(TokenType::Dot, "Expect '.' after 'super'.")?;
        let previous = self.consume(TokenType::Identifier, "Expect superclass method name.")?;
        let name = self.identifier_constant(previous.lexeme(self.source))?;
        self.variable("this", false)?;
        if self.match_type(TokenType::LeftParen) {
            let arity = self.argument_list()?;
            self.variable("super", false)?;
            self.emit_bytes(&[Op::SuperInvoke as u8, name, arity]);
        } else {
            self.variable("super", false)?;
            self.emit_bytes(&[Op::GetSuper as u8, name]);
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
        let previous = self.advance();
        todo!()
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<u8, String> {
        let name = self
            .consume(TokenType::Identifier, error_msg)?
            .lexeme(self.source);
        self.declare_variable(name);
        Ok(if self.current_compiler.scope_depth > 0 {
            0
        } else {
            self.identifier_constant(name)?
        })
    }

    fn mark_initialized(&mut self) {
        self.current_compiler.mark_initialized();
    }

    fn define_variable(&mut self, global: u8) {
        if self.current_compiler.scope_depth > 0 {
            self.current_compiler.mark_initialized();
            return;
        }
        self.emit_bytes(&[Op::DefineGlobal as u8, global])
    }

    fn expression(&mut self) -> Result<(), String> {
        self.parse_precedence(Precedence::Assignment)
    }

    fn block(&mut self) -> Result<(), String> {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::End) {
            self.declaration();
        }
        self.consume(TokenType::RightBrace, "Expect '}' after block.")?;
        Ok(())
    }

    // tot hier

    fn statement(&mut self) -> Result<(), String> {
        !todo!()
    }
    fn declaration(&mut self) {
        !todo!()
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
