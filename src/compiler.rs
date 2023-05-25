use crate::{
    chunk::Op,
    memory::{Heap, Obj},
    object::{Function, Value},
    scanner::{Scanner, Token, TokenType},
};

const U8_COUNT: usize = 0x100;

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
// make this smaller later
#[derive(Clone, Copy)]
struct Local<'src> {
    name: &'src str,
    depth: Option<u16>,
    is_captured: bool,
}

impl<'src> Local<'src> {
    fn new(name: &'src str) -> Self {
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
    upvalues: [Upvalue; U8_COUNT],
    scope_depth: u16,
    // have one local vec, the compiler just keeping offsets?
    locals: [Local<'src>; U8_COUNT],
    local_count: usize,
}

impl<'src> Compiler<'src> {
    fn new(function_type: FunctionType, heap: &mut Heap) -> Self {
        Self {
            function: heap.store(Function::new()),
            upvalues: [Upvalue {
                index: 0,
                is_local: false,
            }; U8_COUNT],
            function_type,
            scope_depth: 0,
            locals: [Local::new(""); U8_COUNT],
            local_count: 0,
        }
    }

    fn resolve_local(&self, name: &str) -> Result<Option<u8>, String> {
        let mut i = self.local_count;
        loop {
            if i == 0 {
                return Ok(None);
            } else {
                i -= 1;
            }
            let local = &self.locals[i];
            if local.name == name {
                return if local.depth.is_none() {
                    Err("Can't read local variable in its own initializer.".to_string())
                } else {
                    Ok(Some(i as u8))
                };
            }
        }
    }

    fn add_local(&mut self, name: &'src str) -> Result<(), String> {
        if self.local_count == U8_COUNT {
            return Err("Too many local variables in function.".to_string());
        }
        self.locals[self.local_count] = Local::new(name);
        self.local_count += 1;
        return Ok(());
    }

    fn mark_initialized(&mut self) -> bool {
        if self.scope_depth == 0 {
            return false;
        }
        let i = self.local_count - 1;
        self.locals[i].depth = Some(self.scope_depth);
        return true;
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<u8, String> {
        let count = self.function.upvalue_count;
        for i in 0..count {
            let upvalue = &self.upvalues[i as usize];
            if upvalue.is_local == is_local && upvalue.index == index {
                return Ok(i as u8);
            }
        }
        if count == u8::MAX {
            return Err("Too many closure variables in function.".to_string());
        }
        self.upvalues[count as usize] = Upvalue { index, is_local };
        self.function.upvalue_count += 1;
        Ok(count as u8)
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

    fn declare_variable(&mut self, name: &'src str) -> Result<(), String> {
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
            Ok(self.advance())
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

pub struct Parser<'src, 'vm> {
    // source
    source: Source<'src>,

    // targets
    compilers: Vec<Compiler<'src>>,

    has_super: bool,
    had_super: Vec<bool>,
    // helper service
    heap: &'vm mut Heap,

    // status
    had_error: bool,
}

impl<'src, 'vm> Parser<'src, 'vm> {
    pub fn new(source: Source<'src>, heap: &'vm mut Heap) -> Self {
        Self {
            source,
            compilers: vec![Compiler::new(FunctionType::Script, heap)],
            has_super: false,
            had_super: Vec::new(),
            heap,
            had_error: false,
        }
    }

    fn current_compiler(&mut self) -> &mut Compiler<'src> {
        let i = self.compilers.len() - 1;
        &mut self.compilers[i]
    }

    fn emit_bytes(&mut self, bytes: &[u8]) {
        let line = self.source.previous_token.line;
        self.current_compiler().function.chunk.write(bytes, line);
    }

    fn emit_loop(&mut self, start: usize) -> Result<(), String> {
        let offset = self.current_compiler().count() - start + 2;
        if offset > u16::MAX as usize {
            Err("loop size to large".to_string())
        } else {
            self.emit_bytes(&[Op::Loop as u8, (offset >> 8) as u8, offset as u8]);
            Ok(())
        }
    }

    fn emit_jump(&mut self, instruction: Op) -> usize {
        self.emit_bytes(&[instruction as u8, 0xff, 0xff]);
        self.current_compiler().count() - 2
    }

    fn emit_return(&mut self) {
        if self.current_compiler().function_type == FunctionType::Initializer {
            self.emit_bytes(&[Op::GetLocal as u8, 0, Op::Return as u8]);
        } else {
            self.emit_bytes(&[Op::Nil as u8, Op::Return as u8]);
        }
    }

    fn emit_constant(&mut self, value: Value) -> Result<(), String> {
        let make_constant = self.current_compiler().make_constant(value)?;
        Ok(self.emit_bytes(&[Op::Constant as u8, make_constant]))
    }

    fn begin_scope(&mut self) {
        self.current_compiler().scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.current_compiler().scope_depth -= 1;
        loop {
            let local_count = self.current_compiler().local_count;
            if local_count == 0 {
                return;
            };
            let local = self.current_compiler().locals[local_count - 1];
            if let Some(depth) = local.depth {
                if depth > self.current_compiler().scope_depth {
                    self.emit_bytes(&[if local.is_captured {
                        Op::CloseUpvalue
                    } else {
                        Op::Pop
                    } as u8]);
                    self.current_compiler().local_count -= 1;
                    continue;
                }
            }
            return;
        }
    }

    fn string_value(&mut self, str: &str) -> Value {
        let downgrade = self.heap.store(str.to_string()).downgrade();
        Value::Obj(downgrade)
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
            self.current_compiler().add_upvalue(index as u8, is_local)?,
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
        let end_jump = self.emit_jump(Op::JumpIfFalse);
        self.emit_bytes(&[Op::Pop as u8]);
        self.parse_precedence(Prec::And)?;
        self.current_compiler().patch_jump(end_jump)
    }

    fn binary(&mut self) -> Result<(), String> {
        match self.source.previous_token.token_type {
            TokenType::BangEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_bytes(&[Op::Equal as u8, Op::Not as u8])
            }
            TokenType::EqualEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_bytes(&[Op::Equal as u8])
            }
            TokenType::Greater => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_bytes(&[Op::Greater as u8])
            }
            TokenType::GreaterEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_bytes(&[Op::Less as u8, Op::Not as u8])
            }
            TokenType::Less => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_bytes(&[Op::Less as u8])
            }
            TokenType::LessEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_bytes(&[Op::Greater as u8, Op::Not as u8])
            }
            TokenType::Plus => {
                self.parse_precedence(Prec::Factor)?;
                self.emit_bytes(&[Op::Add as u8])
            }
            TokenType::Minus => {
                self.parse_precedence(Prec::Factor)?;
                self.emit_bytes(&[Op::Subtract as u8])
            }
            TokenType::Star => {
                self.parse_precedence(Prec::Unary)?;
                self.emit_bytes(&[Op::Multiply as u8])
            }
            TokenType::Slash => {
                self.parse_precedence(Prec::Unary)?;
                self.emit_bytes(&[Op::Divide as u8])
            }
            _ => (), // Unreachable.
        }
        Ok(())
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
        let index = self.current_compiler().make_constant(value)?;
        Ok(index)
    }

    fn lexeme(&self) -> &'src str {
        &self.source.previous_token.lexeme
    }

    fn identifier_name(&mut self, error_msg: &str) -> Result<&'src str, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        Ok(self.lexeme())
    }

    fn identifier_constant(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        self.intern(self.lexeme())
    }

    fn literal(&mut self, token_type: TokenType) {
        self.emit_bytes(&[match token_type {
            TokenType::False => Op::False as u8,
            TokenType::Nil => Op::Nil as u8,
            TokenType::True => Op::True as u8,
            _ => panic!("'{}' mistaken for literal", self.lexeme()),
        }]);
    }

    fn grouping(&mut self) -> Result<(), String> {
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after expression.")?;
        Ok(())
    }

    fn number(&mut self) -> Result<(), String> {
        match self.lexeme().parse::<f64>() {
            Ok(number) => self.emit_constant(Value::Number(number)),
            Err(err) => Err(err.to_string()),
        }
    }

    // I want different logic...
    fn or(&mut self) -> Result<(), String> {
        // no negate top of stack, jump_if, etc.
        let else_jump = self.emit_jump(Op::JumpIfFalse);
        let end_jump = self.emit_jump(Op::Jump);

        self.current_compiler().patch_jump(else_jump)?;
        self.emit_bytes(&[Op::Pop as u8]);

        self.parse_precedence(Prec::Or)?;
        self.current_compiler().patch_jump(end_jump)?;
        Ok(())
    }

    fn string(&mut self) -> Result<(), String> {
        let lexeme = self.lexeme();
        let value = self.string_value(&lexeme[1..lexeme.len() - 1]);
        self.emit_constant(value)
    }

    // admit code for variable access
    fn variable(&mut self, name: &str, can_assign: bool) -> Result<(), String> {
        let (arg, get, set) = {
            if let Some(arg) = self.current_compiler().resolve_local(name)? {
                (arg, Op::GetLocal as u8, Op::SetLocal as u8)
            } else if let Some(arg) = self.resolve_upvalue(name)? {
                (arg, Op::GetUpvalue as u8, Op::SetUpvalue as u8)
            } else {
                let value = self.string_value(name);
                let arg = self.current_compiler().make_constant(value)?;
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
        if self.compilers.is_empty() {
            return Err("Can't use 'this' outside of a class.".to_string());
        }
        self.variable("this", can_assign)
    }

    fn unary(&mut self, token_type: TokenType) -> Result<(), String> {
        self.parse_precedence(Prec::Unary)?;
        match token_type {
            TokenType::Bang => self.emit_bytes(&[Op::Not as u8]),
            TokenType::Minus => self.emit_bytes(&[Op::Negative as u8]),
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
            TokenType::False | TokenType::True | TokenType::Nil => Ok(self.literal(token_type)),
            TokenType::Super => self.super_(),
            TokenType::This => self.this(can_assign),
            _ => Err("Expect expression.".to_string()),
        }
    }

    fn parse_precedence(&mut self, precedence: Prec) -> Result<(), String> {
        self.source.advance();
        let can_assign = precedence <= Prec::Assignment;
        self.parse_prefix(self.source.previous_token.token_type, can_assign)?;

        while precedence <= self.source.current_token.token_type.precedence() {
            self.source.advance();
            self.parse_infix(self.source.previous_token.token_type, can_assign)?;
        }

        if can_assign && self.source.match_type(TokenType::Equal) {
            Err("Invalid assignment target.".to_string())
        } else {
            Ok(())
        }
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        let name = self.lexeme();
        self.current_compiler().declare_variable(name)?;
        Ok(if self.current_compiler().scope_depth > 0 {
            0
        } else {
            let value = self.string_value(name);
            self.current_compiler().make_constant(value)?
        })
    }

    fn define_variable(&mut self, global: u8) {
        if !self.current_compiler().mark_initialized() {
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

    fn function(&mut self, function_type: FunctionType) -> Result<(), String> {
        self.compilers
            .push(Compiler::new(function_type, &mut self.heap));
        self.begin_scope();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after function name.")?;
        if !self.source.check(TokenType::RightParen) {
            loop {
                if self.current_compiler().function.arity == u8::MAX {
                    return Err("Can't have more than 255 parameters.".to_string());
                }
                self.current_compiler().function.arity += 1;
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
        let function = self.compilers.pop().unwrap().function;
        let count = function.upvalue_count;
        let value = Value::Obj(function.downgrade());
        let index = self.current_compiler().make_constant(value)?;
        self.emit_bytes(&[Op::Closure as u8, index]);

        // I don't get this yet
        for i in 0..count {
            let upvalue = self.current_compiler().upvalues[i as usize];
            self.emit_bytes(&[if upvalue.is_local { 1 } else { 0 }, upvalue.index]);
        }
        Ok(())
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
        let class_name = self.identifier_name("Expect class name.")?;
        self.current_compiler().declare_variable(class_name)?;
        let index = self.intern(class_name)?;
        self.emit_bytes(&[Op::Class as u8, index]);
        self.define_variable(index);

        self.had_super.push(self.has_super);
        self.has_super = false;

        // super decl
        if self.source.match_type(TokenType::Less) {
            self.source
                .consume(TokenType::Identifier, "Expect superclass name.")?;
            let super_name = self.lexeme();
            self.variable(super_name, false)?;
            if class_name == super_name {
                return Err("A class can't inherit from itself.".to_string());
            }
            self.begin_scope();
            self.current_compiler().add_local("super")?;
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

        self.has_super = self.had_super.pop().unwrap();
        Ok(())
    }

    fn fun_declaration(&mut self) -> Result<(), String> {
        let index = self.parse_variable("Expect function name.")?;
        self.current_compiler().mark_initialized();
        self.function(FunctionType::Function)?;
        self.define_variable(index);
        Ok(())
    }

    fn var_declaration(&mut self) -> Result<(), String> {
        let index = self.parse_variable("Expect variable name.")?;
        if self.source.match_type(TokenType::Equal) {
            self.expression()?;
        } else {
            self.emit_bytes(&[Op::Nil as u8])
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
        self.emit_bytes(&[Op::Pop as u8]);
        Ok(())
    }

    // hiero
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
        let mut loop_start = self.current_compiler().count();
        let mut exit_jump: Option<usize> = None;
        if !self.source.match_type(TokenType::Semicolon) {
            self.expression()?;
            self.source
                .consume(TokenType::Semicolon, "Expect ';' after loop condition.")?;

            // Jump out of the loop if the condition is false.
            exit_jump = Some(self.emit_jump(Op::JumpIfFalse));
            self.emit_bytes(&[Op::Pop as u8]); // Condition.
        }

        if !self.source.match_type(TokenType::RightParen) {
            let body_jump = self.emit_jump(Op::Jump);
            let increment_start = self.current_compiler().count();
            self.expression()?;
            self.emit_bytes(&[Op::Pop as u8]);
            self.source
                .consume(TokenType::RightParen, "Expect ')' after for clauses.")?;

            self.emit_loop(loop_start)?;
            loop_start = increment_start;
            self.current_compiler().patch_jump(body_jump)?;
        }

        self.statement()?;
        self.emit_loop(loop_start)?;
        if let Some(i) = exit_jump {
            self.current_compiler().patch_jump(i)?;
            self.emit_bytes(&[Op::Pop as u8]);
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
        self.emit_bytes(&[Op::Pop as u8]);
        self.statement()?;
        let else_jump = self.emit_jump(Op::Jump);
        self.current_compiler().patch_jump(then_jump)?;
        self.emit_bytes(&[Op::Pop as u8]);
        if self.source.match_type(TokenType::Else) {
            self.statement()?;
        }
        self.current_compiler().patch_jump(else_jump)?;
        Ok(())
    }

    fn print_statement(&mut self) -> Result<(), String> {
        self.expression()?;
        self.source
            .consume(TokenType::Semicolon, "Expect ';' after value.")?;
        self.emit_bytes(&[Op::Print as u8]);
        Ok(())
    }

    fn return_statement(&mut self) -> Result<(), String> {
        if self.current_compiler().function_type == FunctionType::Script {
            return Err("Can't return from top-level code.".to_string());
        }

        if self.source.match_type(TokenType::Semicolon) {
            self.emit_return();
            Ok(())
        } else {
            if self.current_compiler().function_type == FunctionType::Initializer {
                return Err("Can't return a value from an initializer.".to_string());
            }

            self.expression()?;
            self.source
                .consume(TokenType::Semicolon, "Expect ';' after return value.")?;
            self.emit_bytes(&[Op::Return as u8]);
            Ok(())
        }
    }

    fn while_statement(&mut self) -> Result<(), String> {
        let loop_start = self.current_compiler().count();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after 'while'.")?;
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after condition.")?;

        let exit_jump = self.emit_jump(Op::JumpIfFalse);
        self.emit_bytes(&[Op::Pop as u8]);
        self.statement()?;
        self.emit_loop(loop_start)?;

        self.current_compiler().patch_jump(exit_jump)?;
        self.emit_bytes(&[Op::Pop as u8]);
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
            println!("{msg}");
            self.had_error = true;
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
}

pub fn compile<'src, 'hp>(source: &'src str, heap: &'hp mut Heap) -> Option<Obj<Function>> {
    let mut parser: Parser<'src, 'hp> = Parser::new(Source::new(source), heap);
    while !parser.source.match_type(TokenType::End) {
        parser.declaration();
    }
    let obj = parser.current_compiler().function.clone();
    if parser.had_error {
        None
    } else {
        Some(obj)
    }
}
