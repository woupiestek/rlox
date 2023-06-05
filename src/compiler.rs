use std::mem;

use crate::{
    chunk::{Chunk, Op},
    common::U8_COUNT,
    memory::{Heap, Obj, Traceable},
    object::{Function, Value},
    scanner::{Scanner, Token, TokenType},
};

#[derive(PartialEq, PartialOrd)]
pub enum Prec {
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
}

impl TokenType {
    fn precedence(&self) -> Prec {
        match self {
            TokenType::LeftParen | TokenType::Dot => Prec::Call,
            TokenType::Minus | TokenType::Plus => Prec::Term,
            TokenType::Slash | TokenType::Star => Prec::Factor,
            TokenType::BangEqual | TokenType::EqualEqual => Prec::Equality,
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => Prec::Comparison,
            TokenType::And => Prec::And,
            TokenType::Or => Prec::Or,
            _ => Prec::None,
        }
    }
}

struct Local<'src> {
    name: Token<'src>,
    depth: Option<u16>,
    is_captured: bool,
}

impl<'src> Local<'src> {
    fn new(name: Token<'src>) -> Self {
        Self {
            name,
            depth: None,
            is_captured: false,
        }
    }
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

struct Compiler<'src> {
    function_type: FunctionType,
    function: Obj<Function>,
    // same idea?
    upvalues: Vec<Upvalue>,
    scope_depth: u16,
    // have one local vec, the compiler just keeping offsets?
    locals: Vec<Local<'src>>,
    enclosing: Option<Box<Compiler<'src>>>,
}

impl<'src> Compiler<'src> {
    fn new(function_type: FunctionType, function: Obj<Function>) -> Self {
        let mut first_local = Local::new(Token::synthetic(
            if function_type == FunctionType::Function {
                ""
            } else {
                "this"
            },
        ));
        first_local.depth = Some(0);
        Self {
            function,
            upvalues: Vec::new(),
            function_type,
            scope_depth: 0,
            locals: vec![first_local],
            enclosing: None,
        }
    }

    fn resolve_local(&self, name: &str) -> Result<Option<u8>, String> {
        let mut i = self.locals.len();
        loop {
            if i == 0 {
                return Ok(None);
            } else {
                i -= 1;
            }
            let local = &self.locals[i];
            if local.name.lexeme == name {
                return if local.depth.is_none() {
                    err!(
                        "Can't read local variable '{}' in its own initializer.",
                        local.name.lexeme
                    )
                } else {
                    Ok(Some(i as u8))
                };
            }
        }
    }

    fn add_local(&mut self, name: Token<'src>) -> Result<(), String> {
        if self.locals.len() > U8_COUNT {
            return err!("Too many local variables in function.");
        }
        self.locals.push(Local::new(name));
        Ok(())
    }

    fn mark_initialized(&mut self) -> bool {
        if self.scope_depth == 0 {
            return false;
        }
        let i = self.locals.len() - 1;
        self.locals[i].depth = Some(self.scope_depth);
        true
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<u8, String> {
        let count = self.upvalues.len();
        for i in 0..count {
            let upvalue = &self.upvalues[i];
            if upvalue.is_local == is_local && upvalue.index == index {
                return Ok(i as u8);
            }
        }
        if count > u8::MAX as usize {
            return err!("Too many closure variables in function.");
        }
        self.upvalues.push(Upvalue { index, is_local });
        Ok(count as u8)
    }

    fn resolve_upvalue(&mut self, name: &str) -> Result<Option<u8>, String> {
        let enclosing = match &mut self.enclosing {
            None => {
                return Ok(None);
            }
            Some(compiler) => compiler,
        };

        if let Some(index) = enclosing.resolve_local(name)? {
            enclosing.locals[index as usize].is_captured = true;
            return Ok(Some(self.add_upvalue(index, true)?));
        }

        if let Some(upvalue) = enclosing.resolve_upvalue(name)? {
            return Ok(Some(self.add_upvalue(upvalue, false)?));
        }
        Ok(None)
    }

    fn declare_variable(&mut self, name: Token<'src>) -> Result<(), String> {
        if self.scope_depth == 0 {
            return Ok(());
        }
        let mut i = self.locals.len();
        while i > 0 {
            i -= 1;
            let local = &self.locals[i];
            if let Some(depth) = local.depth {
                if depth < self.scope_depth {
                    break;
                }
            }
            if local.name.lexeme == name.lexeme {
                return Err(format!(
                    "Already a variable with this name in this scope. See line {}, column {}",
                    local.name.line, local.name.column
                ));
            }
        }
        self.add_local(name)
    }
}

pub struct Source<'src> {
    scanner: Scanner<'src>,
    current_token: Token<'src>,
    previous_token: Token<'src>,
}

impl<'src> Source<'src> {
    pub fn new(source: &'src str) -> Self {
        let mut scanner = Scanner::new(source);
        let current_token = scanner.next();
        Self {
            scanner,
            current_token,
            previous_token: Token::nil(),
        }
    }

    fn advance(&mut self) {
        self.previous_token = self.current_token;
        self.current_token = self.scanner.next();
    }

    fn check(&self, token_type: TokenType) -> bool {
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
            self.advance();
            Ok(())
        } else {
            Err(msg)
        }
    }

    fn synchronize(&mut self) {
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
                    return;
                }
                TokenType::Semicolon => {
                    self.advance();
                    return;
                }
                _ => {
                    self.advance();
                    continue;
                }
            }
        }
    }
}

pub struct Parser<'src, 'hp> {
    // source
    source: Source<'src>,

    // targets
    compiler: Box<Compiler<'src>>,

    has_super: u128,
    class_depth: u8,

    // helper service
    heap: &'hp mut Heap,

    // status
    error_count: u8,
}

impl<'src, 'hp> Parser<'src, 'hp> {
    pub fn new(source: Source<'src>, heap: &'hp mut Heap) -> Self {
        Self {
            source,
            compiler: Box::new(Compiler::new(
                FunctionType::Script,
                heap.store(Function::new(None)),
            )),
            has_super: 0,
            class_depth: 0,
            heap,
            error_count: 0,
        }
    }

    fn emit_bytes(&mut self, bytes: &[u8]) {
        let line = self.source.previous_token.line;
        self.current_chunk().write(bytes, line);
    }

    fn emit_op(&mut self, op: Op) {
        let line = self.source.previous_token.line;
        self.current_chunk().write(&[op as u8], line);
    }

    fn emit_loop(&mut self, start: usize) -> Result<(), String> {
        let offset = self.current_chunk().count() - start + 1;
        if offset > u16::MAX as usize {
            err!("loop size to large")
        } else {
            self.emit_bytes(&[Op::Loop as u8, (offset >> 8) as u8, offset as u8]);
            Ok(())
        }
    }

    fn emit_jump(&mut self, instruction: Op) -> usize {
        self.emit_bytes(&[instruction as u8, 0xff, 0xff]);
        self.current_chunk().count() - 2
    }

    fn emit_return(&mut self) {
        if self.compiler.function_type == FunctionType::Initializer {
            self.emit_bytes(&[Op::GetLocal as u8, 0, Op::Return as u8]);
        } else {
            self.emit_bytes(&[Op::Nil as u8, Op::Return as u8]);
        }
    }

    fn emit_constant(&mut self, value: Value) -> Result<(), String> {
        let make_constant = self.current_chunk().add_constant(value)?;
        self.emit_bytes(&[Op::Constant as u8, make_constant]);
        Ok(())
    }

    fn begin_scope(&mut self) {
        self.compiler.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.compiler.scope_depth -= 1;
        let scope_depth = self.compiler.scope_depth;
        loop {
            let is_captured = match self.compiler.locals.last() {
                None => return,
                Some(local) => {
                    if local.depth.is_none() || local.depth.unwrap() <= scope_depth {
                        return;
                    } else {
                        local.is_captured
                    }
                }
            };
            self.emit_bytes(&[if is_captured {
                Op::CloseUpvalue
            } else {
                Op::Pop
            } as u8]);
            self.compiler.locals.pop();
        }
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
                    return err!("Can't have more than 255 arguments.");
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
        let end_jump = self.emit_jump(Op::JumpIfFalse);
        self.emit_op(Op::Pop);
        self.parse_precedence(Prec::And)?;

        self.current_chunk().patch_jump(end_jump)
    }

    fn binary(&mut self) -> Result<(), String> {
        match self.previous_token_type() {
            TokenType::BangEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_bytes(&[Op::Equal as u8, Op::Not as u8])
            }
            TokenType::EqualEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_op(Op::Equal)
            }
            TokenType::Greater => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_op(Op::Greater)
            }
            TokenType::GreaterEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_bytes(&[Op::Less as u8, Op::Not as u8])
            }
            TokenType::Less => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_op(Op::Less)
            }
            TokenType::LessEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_bytes(&[Op::Greater as u8, Op::Not as u8])
            }
            TokenType::Plus => {
                self.parse_precedence(Prec::Factor)?;
                self.emit_op(Op::Add)
            }
            TokenType::Minus => {
                self.parse_precedence(Prec::Factor)?;
                self.emit_op(Op::Subtract)
            }
            TokenType::Star => {
                self.parse_precedence(Prec::Unary)?;
                self.emit_op(Op::Multiply)
            }
            TokenType::Slash => {
                self.parse_precedence(Prec::Unary)?;
                self.emit_op(Op::Divide)
            }
            _ => (), // Unreachable.
        }
        Ok(())
    }

    fn call(&mut self) -> Result<(), String> {
        let arity = self.argument_list()?;
        self.emit_bytes(&[Op::Call as u8, arity]);
        Ok(())
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

    fn intern(&mut self, name: &'src str) -> Result<u8, String> {
        let value = Value::from(self.heap.intern(name));
        self.current_chunk().add_constant(value)
    }

    fn lexeme(&self) -> &'src str {
        self.source.previous_token.lexeme
    }

    fn identifier_constant(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        self.intern(self.lexeme())
    }

    fn grouping(&mut self) -> Result<(), String> {
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after expression.")?;
        Ok(())
    }

    fn number(&mut self) -> Result<(), String> {
        match self.lexeme().parse::<f64>() {
            Ok(number) => self.emit_constant(Value::from(number)),
            Err(err) => Err(err.to_string()),
        }
    }

    fn or(&mut self) -> Result<(), String> {
        let else_jump = self.emit_jump(Op::JumpIfFalse);
        let end_jump = self.emit_jump(Op::Jump);

        self.current_chunk().patch_jump(else_jump)?;
        self.emit_op(Op::Pop);

        self.parse_precedence(Prec::Or)?;

        self.current_chunk().patch_jump(end_jump)?;
        Ok(())
    }

    fn string(&mut self) -> Result<(), String> {
        let lexeme = self.lexeme();
        let value = Value::from(self.heap.intern(&lexeme[1..lexeme.len() - 1]));
        self.emit_constant(value)
    }

    // admit code for variable access
    fn variable(&mut self, name: &'src str, can_assign: bool) -> Result<(), String> {
        let (arg, get, set) = {
            if let Some(arg) = self.compiler.resolve_local(name)? {
                (arg, Op::GetLocal as u8, Op::SetLocal as u8)
            } else if let Some(arg) = self.compiler.resolve_upvalue(name)? {
                (arg, Op::GetUpvalue as u8, Op::SetUpvalue as u8)
            } else {
                let value = Value::from(self.heap.intern(name));
                let arg = self.current_chunk().add_constant(value)?;
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
        if self.class_depth == 0 {
            return err!("Can't use 'super' outside of a class.");
        }
        if self.has_super & 1 == 0 {
            return err!("Can't use 'super' in a class with no superclass.");
        }
        self.source
            .consume(TokenType::Dot, "Expect '.' after 'super'.")?;
        let index = self.identifier_constant("Expect superclass method name.")?;
        self.variable("this", false)?; // and it doesn't friggin' work!
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
        if self.class_depth == 0 {
            return err!("Can't use 'this' outside of a class.");
        }
        self.variable("this", can_assign)
    }

    fn unary(&mut self, token_type: TokenType) -> Result<(), String> {
        self.parse_precedence(Prec::Unary)?;
        match token_type {
            TokenType::Bang => self.emit_op(Op::Not),
            TokenType::Minus => self.emit_op(Op::Negative),
            _ => panic!(),
        }
        Ok(())
    }

    fn parse_infix(&mut self, token_type: TokenType, can_assign: bool) -> Result<(), String> {
        match token_type {
            TokenType::LeftParen => self.call(),
            TokenType::Dot => self.dot(can_assign),
            TokenType::Minus
            | TokenType::Plus
            | TokenType::Slash
            | TokenType::Star
            | TokenType::BangEqual
            | TokenType::EqualEqual
            | TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => self.binary(),
            TokenType::And => self.and(),
            TokenType::Or => self.or(),
            _ => Ok(()), // unreacheable
        }
    }

    fn parse_prefix(&mut self, token_type: TokenType, can_assign: bool) -> Result<(), String> {
        match token_type {
            TokenType::LeftParen => self.grouping(),
            TokenType::Minus | TokenType::Bang => self.unary(token_type),
            TokenType::Identifier => self.variable(self.source.previous_token.lexeme, can_assign),
            TokenType::String => self.string(),
            TokenType::Number => self.number(),
            TokenType::False => {
                self.emit_op(Op::False);
                Ok(())
            }
            TokenType::Nil => {
                self.emit_op(Op::Nil);
                Ok(())
            }
            TokenType::True => {
                self.emit_op(Op::True);
                Ok(())
            }
            TokenType::Super => self.super_(),
            TokenType::This => self.this(can_assign),
            _ => err!("Expect expression."),
        }
    }

    fn parse_precedence(&mut self, precedence: Prec) -> Result<(), String> {
        self.source.advance();
        let can_assign = precedence <= Prec::Assignment;
        self.parse_prefix(self.previous_token_type(), can_assign)?;

        while precedence <= self.source.current_token.token_type.precedence() {
            self.source.advance();
            self.parse_infix(self.previous_token_type(), can_assign)?;
        }

        if can_assign && self.source.match_type(TokenType::Equal) {
            err!("Invalid assignment target.")
        } else {
            Ok(())
        }
    }

    fn previous_token_type(&self) -> TokenType {
        self.source.previous_token.token_type
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        let name = self.source.previous_token;
        self.compiler.declare_variable(name)?;
        if self.compiler.scope_depth > 0 {
            Ok(0)
        } else {
            self.intern(name.lexeme)
        }
    }

    fn define_variable(&mut self, global: u8) {
        if !self.compiler.mark_initialized() {
            self.emit_bytes(&[Op::DefineGlobal as u8, global])
        }
    }

    fn expression(&mut self) -> Result<(), String> {
        self.parse_precedence(Prec::Assignment)
    }

    fn block(&mut self) -> Result<(), String> {
        while !self.source.check(TokenType::RightBrace) && !self.source.check(TokenType::End) {
            self.declaration();
        }
        self.source
            .consume(TokenType::RightBrace, "Expect '}' after block.")?;
        Ok(())
    }

    fn init_compiler(&mut self, function_type: FunctionType) {
        let name = self.heap.intern(self.source.previous_token.lexeme);
        let obj_function = self.heap.store(Function::new(Some(name)));
        let enclosing = mem::replace(
            &mut self.compiler,
            Box::new(Compiler::new(function_type, obj_function)),
        );
        self.compiler.enclosing = Some(enclosing);
    }

    fn end_compiler(&mut self) -> Result<Box<Compiler>, String> {
        self.emit_return();
        let enclosing = self
            .compiler
            .enclosing
            .take()
            .ok_or("Ran out of compilers.")?;
        Ok(mem::replace(&mut self.compiler, enclosing))
    }

    fn function(&mut self, function_type: FunctionType) -> Result<(), String> {
        self.init_compiler(function_type);
        let before = self.compiler.function.byte_count();
        self.begin_scope();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after function name.")?;
        if !self.source.check(TokenType::RightParen) {
            loop {
                if self.compiler.function.arity == u8::MAX {
                    return err!("Can't have more than 255 parameters.");
                }
                (self.compiler.function).arity += 1;
                let index = self.parse_variable("Expect parameter name")?;
                self.define_variable(index);
                if !self.source.match_type(TokenType::Comma) {
                    break;
                }
            }
        }
        self.source
            .consume(TokenType::RightParen, "Expect ')' after parameters.")?;
        self.source
            .consume(TokenType::LeftBrace, "Expect '{' before function body")?;
        self.block()?;

        let compiler = self.end_compiler()?;
        let upvalues = compiler.upvalues;
        let mut obj_function = compiler.function;
        obj_function.upvalue_count = upvalues.len() as u8;
        self.heap
            .increase_byte_count(obj_function.byte_count() - before);
        let index = self
            .current_chunk()
            .add_constant(Value::from(obj_function))?;
        self.emit_bytes(&[Op::Closure as u8, index]);
        // I don't get this yet
        for upvalue in upvalues {
            self.emit_bytes(&[upvalue.is_local as u8, upvalue.index]);
        }
        Ok(())
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.compiler.function.chunk
    }

    fn method(&mut self) -> Result<(), String> {
        self.source
            .consume(TokenType::Identifier, "Expect method name.")?;
        let name = self.lexeme();
        let function_type = if name == "init" {
            FunctionType::Initializer
        } else {
            FunctionType::Method
        };
        let intern = self.intern(name)?;
        self.function(function_type)?;
        self.emit_bytes(&[Op::Method as u8, intern]);
        Ok(())
    }

    //classDecl      → "class" IDENTIFIER ( "<" IDENTIFIER )? "{" function* "}" ;
    fn class(&mut self) -> Result<(), String> {
        self.source
            .consume(TokenType::Identifier, "Expect class name.")?;
        let class_name = self.source.previous_token;
        self.compiler.declare_variable(class_name)?;
        let index = self.intern(class_name.lexeme)?;
        self.emit_bytes(&[Op::Class as u8, index]);
        self.define_variable(index);

        if self.class_depth == 127 {
            return err!("Cannot nest classes that deep");
        }
        self.has_super <<= 1;
        self.class_depth += 1;

        // super decl
        if self.source.match_type(TokenType::Less) {
            self.source
                .consume(TokenType::Identifier, "Expect superclass name.")?;
            let super_name = self.lexeme();
            self.variable(super_name, false)?;
            if class_name.lexeme == super_name {
                return err!("A class can't inherit from itself.");
            }
            self.begin_scope();
            self.compiler.add_local(Token::synthetic("super"))?;
            self.define_variable(0);
            self.variable(class_name.lexeme, false)?;
            self.emit_op(Op::Inherit);
            self.has_super |= 1;
        }

        // why this again?
        self.variable(class_name.lexeme, false)?;

        // class body
        self.source
            .consume(TokenType::LeftBrace, "Expect '{' before class body.")?;
        loop {
            if self.source.match_type(TokenType::RightBrace) {
                break;
            }
            if self.source.check(TokenType::End) {
                return err!("Expect '}}' after class body.");
            }
            self.method()?;
        }
        self.emit_op(Op::Pop);

        if self.has_super & 1 == 1 {
            self.end_scope();
        }

        self.has_super >>= 1;
        self.class_depth -= 1;
        Ok(())
    }

    fn fun_declaration(&mut self) -> Result<(), String> {
        let index = self.parse_variable("Expect function name.")?;
        self.compiler.mark_initialized();
        self.function(FunctionType::Function)?;
        self.define_variable(index);
        Ok(())
    }

    fn var_declaration(&mut self) -> Result<(), String> {
        let index = self.parse_variable("Expect variable name.")?;
        if self.source.match_type(TokenType::Equal) {
            self.expression()?;
        } else {
            self.emit_op(Op::Nil)
        }
        self.source.consume(
            TokenType::Semicolon,
            "Expect ';' after variable declaration.",
        )?;
        self.define_variable(index);
        Ok(())
    }

    fn expression_statement(&mut self) -> Result<(), String> {
        self.expression()?;
        self.source
            .consume(TokenType::Semicolon, "Expect ';' after expression.")?;
        self.emit_op(Op::Pop);
        Ok(())
    }

    fn for_statement(&mut self) -> Result<(), String> {
        self.begin_scope();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after 'for'.")?;
        if !self.source.match_type(TokenType::Semicolon) {
            if self.source.match_type(TokenType::Var) {
                self.var_declaration()
            } else {
                self.expression_statement()
            }?;
        }
        let mut loop_start = self.current_chunk().count();
        let mut exit_jump: Option<usize> = None;
        if !self.source.match_type(TokenType::Semicolon) {
            self.expression()?;
            self.source
                .consume(TokenType::Semicolon, "Expect ';' after loop condition.")?;

            // Jump out of the loop if the condition is false.
            exit_jump = Some(self.emit_jump(Op::JumpIfFalse));
            self.emit_op(Op::Pop); // Condition.
        }

        if !self.source.match_type(TokenType::RightParen) {
            let body_jump = self.emit_jump(Op::Jump);
            let increment_start = self.current_chunk().count();
            self.expression()?;
            self.emit_op(Op::Pop);
            self.source
                .consume(TokenType::RightParen, "Expect ')' after for clauses.")?;

            self.emit_loop(loop_start)?;
            loop_start = increment_start;

            self.current_chunk().patch_jump(body_jump)?;
        }

        self.statement()?;
        self.emit_loop(loop_start)?;
        if let Some(i) = exit_jump {
            self.current_chunk().patch_jump(i)?;
            self.emit_op(Op::Pop);
        }
        self.end_scope();
        Ok(())
    }

    fn if_statement(&mut self) -> Result<(), String> {
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after 'if'.")?;
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after condition.")?;

        let then_jump = self.emit_jump(Op::JumpIfFalse);
        self.emit_op(Op::Pop);
        self.statement()?;
        let else_jump = self.emit_jump(Op::Jump);

        self.current_chunk().patch_jump(then_jump)?;
        self.emit_op(Op::Pop);
        if self.source.match_type(TokenType::Else) {
            self.statement()?;
        }

        self.current_chunk().patch_jump(else_jump)?;
        Ok(())
    }

    fn print_statement(&mut self) -> Result<(), String> {
        self.expression()?;
        self.source
            .consume(TokenType::Semicolon, "Expect ';' after value.")?;
        self.emit_op(Op::Print);
        Ok(())
    }

    fn return_statement(&mut self) -> Result<(), String> {
        if self.compiler.function_type == FunctionType::Script {
            return err!("Can't return from top-level code.");
        }

        if self.source.match_type(TokenType::Semicolon) {
            self.emit_return();
            Ok(())
        } else {
            if self.compiler.function_type == FunctionType::Initializer {
                return err!("Can't return a value from an initializer.");
            }

            self.expression()?;
            self.source
                .consume(TokenType::Semicolon, "Expect ';' after return value.")?;
            self.emit_op(Op::Return);
            Ok(())
        }
    }

    fn while_statement(&mut self) -> Result<(), String> {
        let loop_start = self.current_chunk().count();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after 'while'.")?;
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after condition.")?;

        let exit_jump = self.emit_jump(Op::JumpIfFalse);
        self.emit_op(Op::Pop);
        self.statement()?;
        self.emit_loop(loop_start)?;

        self.current_chunk().patch_jump(exit_jump)?;
        self.emit_op(Op::Pop);
        Ok(())
    }

    fn declaration(&mut self) {
        let result = if self.source.match_type(TokenType::Class) {
            self.class()
        } else if self.source.match_type(TokenType::Fun) {
            self.fun_declaration()
        } else if self.source.match_type(TokenType::Var) {
            self.var_declaration()
        } else {
            self.statement()
        };

        if let Err(msg) = result {
            // I know, don't log & throw
            // also: what about an actual logger?
            println!(
                "[line: {}, column: {}, lexeme: {}] {}",
                self.source.previous_token.line,
                self.source.previous_token.column,
                self.source.previous_token.lexeme,
                msg
            );
            self.error_count += 1;
            self.source.synchronize();
        }
    }

    fn statement(&mut self) -> Result<(), String> {
        if self.source.match_type(TokenType::Print) {
            self.print_statement()
        } else if self.source.match_type(TokenType::For) {
            self.for_statement()
        } else if self.source.match_type(TokenType::If) {
            self.if_statement()
        } else if self.source.match_type(TokenType::Return) {
            self.return_statement()
        } else if self.source.match_type(TokenType::While) {
            self.while_statement()
        } else if self.source.match_type(TokenType::LeftBrace) {
            self.begin_scope();
            let result = self.block();
            self.end_scope();
            result
        } else {
            self.expression_statement()
        }
    }

    fn script(&mut self) -> Result<Obj<Function>, String> {
        let before = self.compiler.function.byte_count();
        while !self.source.match_type(TokenType::End) {
            self.declaration();
        }
        self.emit_return();
        let replace = mem::replace(
            &mut self.compiler,
            Box::new(Compiler::new(
                FunctionType::Script,
                self.heap.store(Function::new(None)),
            )),
        )
        .function;

        self.heap.increase_byte_count(replace.byte_count() - before);
        Ok(replace)
    }
}

pub fn compile<'src, 'hp>(source: &'src str, heap: &'hp mut Heap) -> Result<Obj<Function>, String> {
    let mut parser: Parser<'src, 'hp> = Parser::new(Source::new(source), heap);
    let obj = parser.script()?;
    match parser.error_count {
        0 => Ok(obj),
        1 => err!("There was a compile time error."),
        more => err!("There were {} compile time errors.", more),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! disassemble {
        ($chunk:expr) => {
            #[cfg(feature = "trace")]
            {
                use crate::debug::Disassembler;
                Disassembler::disassemble($chunk);
            }
        };
    }

    #[test]
    fn construct_parser<'src>() {
        let source = Source::new("");
        Parser::new(source, &mut Heap::new());
    }

    #[test]
    fn parse_empty_string() {
        let mut heap = Heap::new();
        let mut parser = Parser::new(Source::new(""), &mut heap);
        assert!(parser.source.match_type(TokenType::End));
    }

    #[test]
    fn compile_empty_string() {
        let result = compile("", &mut Heap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn scoping() {
        let test = "{
            var a = \"outer a\";
            var b = \"outer b\";
            {
              var a = \"inner a\";
              print a;
              print b;
              print c;
            }
            print a;
            print b;
            print c;
          }";
        let result = compile(test, &mut Heap::new());
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn scoping_2() {
        let test = "fun add(a, b, c) {
            print a + b + c;
          }
          
          add(1, 2, 3);
          
          fun add(a, b) {
            print a + b;
          }
          
          print add; // \"<fn add>\".
          ";
        let result = compile(test, &mut Heap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn scoping_3() {
        let test = "var a = \"global\";
        {
          fun showA() {
            print a;
          }
        
          showA();
          var a = \"block\";
          showA();
        }
        var a = 1;
        ";
        let result = compile(test, &mut Heap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn disassemble() {
        let test = "var a = 1;
        var b = 2;
        print a + b;";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn printing() {
        let test = "print \"hi\"; // \"hi\".";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn boolean_logic() {
        let test = "print \"hi\" or 2; // \"hi\".";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn for_loop_long() {
        let test = "
        var a = 0;
        var temp;
        for (var b = 1; a < 10000; b = temp + b) {
            print a;
            temp = a;
            a = b;
        }";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn for_loop_short() {
        let test = "
        for (var b = 0; b < 10; b = b + 1) {
            print \"test\";
        }";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn identity_function() {
        let test = "fun id(x) { return x; }";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn function_calls() {
        let test = "
        fun sayHi(first, last) {
            print \"Hi, \" + first + \" \" + last + \"!\";
          }
          
          sayHi(\"Dear\", \"Reader\");
          
          fun add(a, b, c) {
            print a + b + c;
          }
          
          add(1, 2, 3);
        ";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn local_variable_initializer() {
        let test = "
        class Cake {
            taste() {
              var adjective = \"delicious\";
              print \"The \" + this.flavor + \" cake is \" + adjective + \"!\";
            }
          }
                  ";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn too_many_constants() {
        let test = "
        var a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;

        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;

        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;

        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        a;a;a;a; a;a;a;a; a;a;a;a; a;a;a;a;
        ";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn super_call() {
        let test = "
        class A {
            f(x) { print x; }
        }
        class B < A {
            f(x) { super.f(x); print x; }
        }
        B.f(\"hello\");
        ";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }
}
