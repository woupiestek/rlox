use crate::{
    chunk::Op,
    common::U8_COUNT,
    memory::{Heap, Obj},
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
// make this smaller later
#[derive(Clone, Copy)]
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
}

impl<'src> Compiler<'src> {
    fn new(function_type: FunctionType, heap: &mut Heap) -> Self {
        Self {
            function: heap.store(Function::new()),
            upvalues: Vec::new(),
            function_type,
            scope_depth: 0,
            locals: Vec::new(),
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
                    err!("Can't read local variable in its own initializer.")
                } else {
                    Ok(Some(i as u8))
                };
            }
        }
    }

    fn add_local(&mut self, name: Token<'src>) -> Result<(), String> {
        if self.locals.len() == U8_COUNT {
            return err!("Too many local variables in function.");
        }
        self.locals.push(Local::new(name));
        return Ok(());
    }

    fn mark_initialized(&mut self) -> bool {
        if self.scope_depth == 0 {
            return false;
        }
        let i = self.locals.len() - 1;
        self.locals[i].depth = Some(self.scope_depth);
        return true;
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<u8, String> {
        let count = self.upvalues.len();
        for i in 0..count {
            let upvalue = &self.upvalues[i as usize];
            if upvalue.is_local == is_local && upvalue.index == index {
                return Ok(i as u8);
            }
        }
        if count == u8::MAX as usize {
            return err!("Too many closure variables in function.");
        }
        self.upvalues.push(Upvalue { index, is_local });
        self.function.upvalue_count = self.upvalues.len() as u8;
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
            err!("jump too large")
        } else {
            self.function
                .chunk
                .over_write(&[(jump >> 8) as u8, jump as u8], offset);
            Ok(())
        }
    }

    fn declare_variable(&mut self, name: Token<'src>) -> Result<(), String> {
        if self.scope_depth == 0 {
            return Ok(());
        }
        let mut i = self.locals.len();
        while i > 0 {
            i -= 1;
            let local = self.locals[i];
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

pub struct Parser<'src, 'hp> {
    // source
    source: Source<'src>,

    // targets
    compilers: Vec<Compiler<'src>>,

    has_super: u128,
    class_depth: u8,

    // helper service
    heap: &'hp mut Heap,

    // status
    had_error: Option<String>,
}

impl<'src, 'hp> Parser<'src, 'hp> {
    pub fn new(source: Source<'src>, heap: &'hp mut Heap) -> Self {
        Self {
            source,
            compilers: vec![Compiler::new(FunctionType::Script, heap)],
            has_super: 0,
            class_depth: 0,
            heap,
            had_error: None,
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

    fn emit_op(&mut self, op: Op) {
        let line = self.source.previous_token.line;
        self.current_compiler()
            .function
            .chunk
            .write(&[op as u8], line);
    }

    fn emit_loop(&mut self, start: usize) -> Result<(), String> {
        let offset = self.current_compiler().count() - start + 2;
        if offset > u16::MAX as usize {
            err!("loop size to large")
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
        let scope_depth = self.current_compiler().scope_depth;
        loop {
            match self.current_compiler().locals.last() {
                None => return,
                Some(local) => {
                    if local.depth.is_none() || local.depth.unwrap() <= scope_depth {
                        return;
                    }
                }
            }
            let local = self.current_compiler().locals.pop().unwrap();
            let depth = local.depth.unwrap();
            if depth > scope_depth {
                self.emit_bytes(&[if local.is_captured {
                    Op::CloseUpvalue
                } else {
                    Op::Pop
                } as u8]);
                self.current_compiler().locals.pop();
                continue;
            }
        }
    }

    fn string_value(&mut self, str: &'src str) -> Value {
        let downgrade = self.heap.intern(str).as_handle();
        Value::Object(downgrade)
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

    fn intern(&mut self, name: &'src str) -> Result<u8, String> {
        let value = self.string_value(name);
        let index = self.current_compiler().make_constant(value)?;
        Ok(index)
    }

    fn lexeme(&self) -> &'src str {
        &self.source.previous_token.lexeme
    }

    fn identifier_name(&mut self, error_msg: &str) -> Result<Token<'src>, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        Ok(self.source.previous_token)
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
        self.emit_op(Op::Pop);

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
    fn variable(&mut self, name: &'src str, can_assign: bool) -> Result<(), String> {
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
        if self.compilers.is_empty() {
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
            TokenType::False => Ok(self.emit_op(Op::False)),
            TokenType::Nil => Ok(self.emit_op(Op::Nil)),
            TokenType::True => Ok(self.emit_op(Op::True)),
            TokenType::Super => self.super_(),
            TokenType::This => self.this(can_assign),
            _ => err!("Expect expression."),
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
            err!("Invalid assignment target.")
        } else {
            Ok(())
        }
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        let name = self.source.previous_token;
        self.current_compiler().declare_variable(name)?;
        Ok(if self.current_compiler().scope_depth > 0 {
            0
        } else {
            let value = self.string_value(name.lexeme);
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

    fn end_compiler(&mut self) -> Compiler {
        self.emit_return();
        self.compilers.pop().unwrap()
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
                    return err!("Can't have more than 255 parameters.");
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

        let compiler = self.end_compiler();
        let function = compiler.function;
        let upvalues = compiler.upvalues;
        let index = self.current_compiler().make_constant(function.as_value())?;
        self.emit_bytes(&[Op::Closure as u8, index]);

        // I don't get this yet
        for i in 0..function.upvalue_count {
            let upvalue = upvalues[i as usize];
            self.emit_bytes(&[upvalue.is_local as u8, upvalue.index]);
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
            self.current_compiler()
                .add_local(Token::synthetic("super"))?;
            self.define_variable(0);
            self.variable(class_name.lexeme, false)?;
            self.emit_op(Op::Inherit);
            self.has_super &= 1;
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
        let mut loop_start = self.current_compiler().count();
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
            let increment_start = self.current_compiler().count();
            self.expression()?;
            self.emit_op(Op::Pop);
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
        self.current_compiler().patch_jump(then_jump)?;
        self.emit_op(Op::Pop);
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
        self.emit_op(Op::Print);
        Ok(())
    }

    fn return_statement(&mut self) -> Result<(), String> {
        if self.current_compiler().function_type == FunctionType::Script {
            return err!("Can't return from top-level code.");
        }

        if self.source.match_type(TokenType::Semicolon) {
            self.emit_return();
            Ok(())
        } else {
            if self.current_compiler().function_type == FunctionType::Initializer {
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
        let loop_start = self.current_compiler().count();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after 'while'.")?;
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after condition.")?;

        let exit_jump = self.emit_jump(Op::JumpIfFalse);
        self.emit_op(Op::Pop);
        self.statement()?;
        self.emit_loop(loop_start)?;

        self.current_compiler().patch_jump(exit_jump)?;
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
                "[lint:{},column:{}] {}",
                self.source.previous_token.line, self.source.previous_token.column, msg
            );
            // maybe collect errors in a vec?
            self.had_error = Some(msg);
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

pub fn compile<'src, 'hp>(source: &'src str, heap: &'hp mut Heap) -> Result<Obj<Function>, String> {
    let mut parser: Parser<'src, 'hp> = Parser::new(Source::new(source), heap);
    while !parser.source.match_type(TokenType::End) {
        parser.declaration();
    }
    let obj = parser.end_compiler().function;
    if let Some(msg) = parser.had_error {
        Err(msg)
    } else {
        Ok(obj)
    }
}

#[cfg(test)]
mod tests {

    use crate::debug::Disassembler;

    use super::*;

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
    fn for_loop() {
        let test = "
        for (var b = 1; a < 10000; b = temp + b) {
          print a;
          temp = a;
          a = b;
        }
        ";
        let result = compile(test, &mut Heap::new());
        assert!(result.is_ok());
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
        Disassembler::disassemble(&result.unwrap().chunk);
    }
}
