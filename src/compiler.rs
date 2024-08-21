use std::{mem, time::Instant, usize};

use crate::{
    chunk::{Chunk, Op},
    loxtr::Loxtr,
    memory::{Heap, Traceable, GC},
    object::{Function, Value},
    scanner::{Scanner, TokenType},
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

struct Local {
    offset: usize,
    depth: Option<u16>,
    is_captured: bool,
}

impl Local {
    fn new(offset: usize) -> Self {
        Self {
            offset,
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum FunctionType {
    Function,
    Initializer,
    Method,
    Script,
}

struct CompileData {
    function_type: FunctionType,
    function: GC<Function>,
    upvalues: Vec<Upvalue>,
    scope_depth: u16,
    locals: Vec<Local>,
}

impl CompileData {
    fn new(function_type: FunctionType, function: GC<Function>) -> Self {
        let mut first_local = Local::new(THIS_OFFSET);
        first_local.depth = Some(0);
        Self {
            function,
            upvalues: Vec::new(),
            function_type,
            scope_depth: 0,
            locals: vec![first_local],
        }
    }
}

struct Compiler<'src, 'hp> {
    head: CompileData,
    tail: Vec<CompileData>,
    source: Source<'src>,
    heap: &'hp mut Heap,
}

const THIS_OFFSET: usize = usize::MAX;
const SUPER_OFFSET: usize = THIS_OFFSET - 1;

impl<'src, 'hp> Compiler<'src, 'hp> {
    fn new(
        function_type: FunctionType,
        function: GC<Function>,
        source: Source<'src>,
        heap: &'hp mut Heap,
    ) -> Self {
        Self {
            head: CompileData::new(function_type, function),
            tail: Vec::new(),
            source,
            heap,
        }
    }

    fn get_offset_name(&self, offset: usize) -> Result<&str, String> {
        if offset == THIS_OFFSET {
            if self.head.function_type == FunctionType::Function {
                return Ok("");
            }
            return Ok("this");
        }
        if offset == SUPER_OFFSET {
            return Ok("super");
        }
        self.source.scanner.get_identifier_name(offset)
    }

    fn data(&self, depth: usize) -> &CompileData {
        if depth == 0 {
            &self.head
        } else {
            &self.tail[self.tail.len() - depth]
        }
    }

    fn mut_data(&mut self, depth: usize) -> &mut CompileData {
        if depth == 0 {
            &mut self.head
        } else {
            let index = self.tail.len() - depth;
            &mut self.tail[index]
        }
    }

    fn resolve_local(&self, depth: usize, offset: usize) -> Result<Option<u8>, String> {
        let mut i = self.data(depth).locals.len();
        loop {
            if i == 0 {
                return Ok(None);
            } else {
                i -= 1;
            }
            let local = &self.head.locals[i];
            let local_name = self.get_offset_name(local.offset)?;
            if local_name == self.get_offset_name(offset)? {
                return if local.depth.is_none() {
                    err!(
                        "Can't read local variable '{}' in its own initializer.",
                        local_name
                    )
                } else {
                    Ok(Some(i as u8))
                };
            }
        }
    }

    fn add_local(&mut self, name: usize) -> Result<(), String> {
        if self.head.locals.len() > u8::MAX as usize {
            return err!("Too many local variables in function.");
        }
        self.head.locals.push(Local::new(name));
        Ok(())
    }

    fn mark_initialized(&mut self) -> bool {
        if self.head.scope_depth == 0 {
            return false;
        }
        let i = self.head.locals.len() - 1;
        self.head.locals[i].depth = Some(self.head.scope_depth);
        true
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<u8, String> {
        let count = self.head.upvalues.len();
        for i in 0..count {
            let upvalue = &self.head.upvalues[i];
            if upvalue.is_local == is_local && upvalue.index == index {
                return Ok(i as u8);
            }
        }
        if count > u8::MAX as usize {
            return err!("Too many closure variables in function.");
        }
        self.head.upvalues.push(Upvalue { index, is_local });
        Ok(count as u8)
    }

    fn resolve_upvalue(&mut self, depth: usize, offset: usize) -> Result<Option<u8>, String> {
        if self.tail.len() < depth {
            return Ok(None);
        }

        if let Some(index) = self.resolve_local(depth + 1, offset)? {
            self.mut_data(depth + 1).locals[index as usize].is_captured = true;
            return Ok(Some(self.add_upvalue(index, true)?));
        }

        if let Some(upvalue) = self.resolve_upvalue(depth + 1, offset)? {
            return Ok(Some(self.add_upvalue(upvalue, false)?));
        }
        Ok(None)
    }

    fn declare_variable(&mut self, name: usize) -> Result<(), String> {
        if self.head.scope_depth == 0 {
            return Ok(());
        }
        let mut i = self.head.locals.len();
        while i > 0 {
            i -= 1;
            let local = &self.head.locals[i];
            if let Some(depth) = local.depth {
                if depth < self.head.scope_depth {
                    break;
                }
            }
            let local_name = self.get_offset_name(local.offset)?;
            let identifier_name = self.source.scanner.get_identifier_name(name)?;
            if local_name == identifier_name {
                let (l, c) = self.source.scanner.line_and_column(local.offset);
                return Err(format!(
                    "Already a variable with this name in this scope. See line {}, column {}",
                    l, c
                ));
            }
        }
        self.add_local(name)
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.head.function.chunk
    }

    fn emit_return(&mut self) {
        if self.head.function_type == FunctionType::Initializer {
            self.emit_byte_op(Op::GetLocal, 0);
        } else {
            self.emit_op(Op::Nil);
        }
        self.emit_op(Op::Return);
    }

    fn emit_byte_op(&mut self, op: Op, byte: u8) {
        let line = self.previous_token_line();
        self.current_chunk().write_byte_op(op, byte, line);
    }

    fn emit_short_op(&mut self, op: Op, short: u16) {
        let line = self.previous_token_line();
        self.current_chunk().write_short_op(op, short, line);
    }

    fn emit_invoke_op(&mut self, op: Op, constant: u8, arity: u8) {
        let line = self.previous_token_line();
        self.current_chunk()
            .write_invoke_op(op, constant, arity, line);
    }

    fn emit_op(&mut self, op: Op) {
        let line = self.previous_token_line();
        self.current_chunk().write(&[op as u8], line);
    }

    fn emit_loop(&mut self, start: usize) -> Result<(), String> {
        let offset = self.current_chunk().count() - start + 1;
        if offset > u16::MAX as usize {
            err!("loop size to large")
        } else {
            self.emit_short_op(Op::Loop, offset as u16);
            Ok(())
        }
    }

    fn emit_jump(&mut self, instruction: Op) -> usize {
        self.emit_short_op(instruction, 0xffff);
        self.current_chunk().count() - 2
    }

    fn emit_constant(&mut self, value: Value) -> Result<(), String> {
        let make_constant = self.current_chunk().add_constant(value)?;
        self.emit_byte_op(Op::Constant, make_constant);
        Ok(())
    }

    fn begin_scope(&mut self) {
        self.head.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.head.scope_depth -= 1;
        let scope_depth = self.head.scope_depth;
        loop {
            let is_captured = match self.head.locals.last() {
                None => return,
                Some(local) => {
                    if local.depth.is_none() || local.depth.unwrap() <= scope_depth {
                        return;
                    } else {
                        local.is_captured
                    }
                }
            };
            self.emit_op(if is_captured {
                Op::CloseUpvalue
            } else {
                Op::Pop
            });
            self.head.locals.pop();
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
        match self.source.previous_type {
            TokenType::BangEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_op(Op::Equal);
                self.emit_op(Op::Not);
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
                self.emit_op(Op::Less);
                self.emit_op(Op::Not);
            }
            TokenType::Less => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_op(Op::Less)
            }
            TokenType::LessEqual => {
                self.parse_precedence(Prec::Equality)?;
                self.emit_op(Op::Greater);
                self.emit_op(Op::Not);
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
        self.emit_byte_op(Op::Call, arity);
        Ok(())
    }

    fn dot(&mut self, can_assign: bool) -> Result<(), String> {
        let index = self.identifier_constant("Expect property name after '.'.")?;
        if can_assign && self.source.match_type(TokenType::Equal) {
            self.expression()?;
            self.emit_byte_op(Op::SetProperty, index)
        } else if self.source.match_type(TokenType::LeftParen) {
            let arity = self.argument_list()?;
            self.emit_invoke_op(Op::Invoke, index, arity);
        } else {
            self.emit_byte_op(Op::GetProperty, index);
        };
        Ok(())
    }

    fn number(&mut self) -> Result<(), String> {
        match self.source.scanner.get_number(self.source.previous_token) {
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
        let value = self
            .heap
            .intern_copy(self.source.scanner.get_str(self.source.previous_token)?);
        self.emit_constant(Value::from(value))
    }

    // admit code for variable access
    fn variable(&mut self, offset: usize, can_assign: bool) -> Result<(), String> {
        let (arg, get, set) = {
            if let Some(arg) = self.resolve_local(0, offset)? {
                (arg, Op::GetLocal, Op::SetLocal)
            } else if let Some(arg) = self.resolve_upvalue(0, offset)? {
                (arg, Op::GetUpvalue, Op::SetUpvalue)
            } else {
                let value = self.heap.intern(self.get_offset_name(offset)?.to_owned());
                let arg = self.current_chunk().add_constant(Value::from(value))?;
                (arg, Op::GetGlobal, Op::SetGlobal)
            }
        };

        if can_assign && self.source.match_type(TokenType::Equal) {
            self.expression()?;
            self.emit_byte_op(set, arg);
        } else {
            self.emit_byte_op(get, arg);
        }
        Ok(())
    }

    fn super_(&mut self) -> Result<(), String> {
        if self.source.class_depth == 0 {
            return err!("Can't use 'super' outside of a class.");
        }
        if self.source.has_super & 1 == 0 {
            return err!("Can't use 'super' in a class with no superclass.");
        }
        self.source
            .consume(TokenType::Dot, "Expect '.' after 'super'.")?;
        let index = self.identifier_constant("Expect superclass method name.")?;
        self.variable(THIS_OFFSET, false)?;
        if self.source.match_type(TokenType::LeftParen) {
            let arity = self.argument_list()?;
            self.variable(SUPER_OFFSET, false)?;
            self.emit_invoke_op(Op::SuperInvoke, index, arity);
        } else {
            self.variable(SUPER_OFFSET, false)?;
            self.emit_byte_op(Op::GetSuper, index);
        }
        Ok(())
    }

    fn this(&mut self, can_assign: bool) -> Result<(), String> {
        if self.source.class_depth == 0 {
            return err!("Can't use 'this' outside of a class.");
        }
        self.variable(THIS_OFFSET, can_assign)
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
            TokenType::Identifier => self.variable(self.source.previous_token, can_assign),
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
        self.parse_prefix(self.source.previous_type, can_assign)?;

        while precedence <= self.source.current_type.precedence() {
            self.source.advance();
            self.parse_infix(self.source.previous_type, can_assign)?;
        }

        if can_assign && self.source.match_type(TokenType::Equal) {
            err!("Invalid assignment target.")
        } else {
            Ok(())
        }
    }

    fn intern_identifier_at(&mut self, offset: usize) -> Result<u8, String> {
        let name = self.source.scanner.get_identifier_name(offset)?;
        let heap = self.heap.intern_copy(name);
        self.intern(heap)
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        let name = self.source.previous_token;
        self.declare_variable(name)?;
        if self.head.scope_depth > 0 {
            Ok(0)
        } else {
            self.intern_identifier_at(name)
        }
    }

    fn define_variable(&mut self, global: u8) {
        if !self.mark_initialized() {
            self.emit_byte_op(Op::DefineGlobal, global)
        }
    }

    fn expression(&mut self) -> Result<(), String> {
        self.parse_precedence(Prec::Assignment)
    }

    fn grouping(&mut self) -> Result<(), String> {
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after expression.")
    }

    fn function_body(&mut self) -> Result<(), String> {
        self.begin_scope();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after function name.")?;
        if !self.source.check(TokenType::RightParen) {
            loop {
                if self.head.function.arity == u8::MAX {
                    return err!("Can't have more than 255 parameters.");
                }
                (self.head.function).arity += 1;
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
        self.emit_return();
        Ok(())
    }

    fn previous_token_line(&self) -> u16 {
        self.source
            .scanner
            .line_and_column(self.source.previous_token)
            .0
    }

    fn function(&mut self, function_type: FunctionType) -> Result<(), String> {
        let name = self
            .source
            .scanner
            .get_identifier_name(self.source.previous_token)?;
        let name = self.heap.intern_copy(name);
        let mut function = self.heap.store(Function::new(Some(name)));
        let before = function.byte_count();
        self.tail.push(mem::replace(
            &mut self.head,
            CompileData::new(function_type, function),
        ));
        self.function_body()?;
        self.head = self.tail.pop().unwrap();

        let upvalues = CompileData::new(function_type, function).upvalues;
        function.upvalue_count = upvalues.len() as u8;
        self.heap
            .increase_byte_count(function.byte_count() - before);
        let index = self.current_chunk().add_constant(Value::from(function))?;
        self.emit_byte_op(Op::Closure, index);
        let line = self.previous_token_line();
        for upvalue in upvalues {
            self.current_chunk()
                .write(&[upvalue.is_local as u8, upvalue.index], line);
        }
        Ok(())
    }

    fn method(&mut self) -> Result<(), String> {
        self.source
            .consume(TokenType::Identifier, "Expect method name.")?;
        let name = self.source.identifier_name()?;
        let function_type = if name == "init" {
            FunctionType::Initializer
        } else {
            FunctionType::Method
        };
        let loxtr = self.heap.intern_copy(name);
        let intern = self.intern(loxtr)?;
        self.function(function_type)?;
        self.emit_byte_op(Op::Method, intern);
        Ok(())
    }

    fn class(&mut self) -> Result<(), String> {
        self.source
            .consume(TokenType::Identifier, "Expect class name.")?;
        let class_offset = self.source.previous_token;
        self.declare_variable(class_offset)?;
        let index = self.intern_identifier_at(class_offset)?;
        self.emit_byte_op(Op::Class, index);
        self.define_variable(index);

        if self.source.class_depth == 127 {
            return err!("Cannot nest classes that deep");
        }
        self.source.has_super <<= 1;
        self.source.class_depth += 1;

        // super decl
        if self.source.match_type(TokenType::Less) {
            self.source
                .consume(TokenType::Identifier, "Expect superclass name.")?;
            self.variable(SUPER_OFFSET, false)?;
            if self.source.scanner.get_identifier_name(class_offset)?
                == self.source.identifier_name()?
            {
                return err!("A class can't inherit from itself.");
            }
            self.begin_scope();
            self.add_local(SUPER_OFFSET)?;
            self.define_variable(0);
            self.variable(class_offset, false)?;
            self.emit_op(Op::Inherit);
            self.source.has_super |= 1;
        }

        // why this again?
        self.variable(class_offset, false)?;

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

        if self.source.has_super & 1 == 1 {
            self.end_scope();
        }

        self.source.has_super >>= 1;
        self.source.class_depth -= 1;
        Ok(())
    }

    fn fun_declaration(&mut self) -> Result<(), String> {
        let index = self.parse_variable("Expect function name.")?;
        self.mark_initialized();
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
        if self.head.function_type == FunctionType::Script {
            return err!("Can't return from top-level code.");
        }

        if self.source.match_type(TokenType::Semicolon) {
            self.emit_return();
            Ok(())
        } else {
            if self.head.function_type == FunctionType::Initializer {
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

    fn intern(&mut self, loxtr: GC<Loxtr>) -> Result<u8, String> {
        self.current_chunk().add_constant(Value::from(loxtr))
    }

    fn identifier_constant(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        self.intern_identifier_at(self.source.previous_token)
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
            let (l, c) = self
                .source
                .scanner
                .line_and_column(self.source.previous_token);
            println!("[line: {}, column: {}] {}", l, c, msg);
            self.source.error_count += 1;
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

    fn script(&mut self) -> Result<GC<Function>, String> {
        let before = self.head.function.byte_count();
        while !self.source.match_type(TokenType::End) {
            self.declaration();
        }
        self.emit_return();
        let replace = self.head.function;
        self.heap.increase_byte_count(replace.byte_count() - before);
        Ok(replace)
    }

    fn block(&mut self) -> Result<(), String> {
        while !self.source.check(TokenType::RightBrace) && !self.source.check(TokenType::End) {
            self.declaration();
        }
        self.source
            .consume(TokenType::RightBrace, "Expect '}' after block.")?;
        Ok(())
    }
}

pub struct Source<'src> {
    scanner: Scanner<'src>,
    current_type: TokenType,
    current_token: usize,
    previous_type: TokenType,
    previous_token: usize,

    has_super: u128,
    class_depth: u8,

    // helper service
    // heap: &'hp mut Heap,

    // status
    error_count: u8,
}

impl<'src> Source<'src> {
    pub fn new(source: &'src str) -> Self {
        let mut scanner = Scanner::new(source);
        let (current_type, current_token) = scanner.next();
        Self {
            scanner,
            current_token,
            current_type,
            previous_token: usize::MAX,
            previous_type: TokenType::Error,
            has_super: 0,
            class_depth: 0,
            error_count: 0,
        }
    }

    fn advance(&mut self) {
        self.previous_token = self.current_token;
        (self.current_type, self.current_token) = self.scanner.next();
    }

    fn check(&self, token_type: TokenType) -> bool {
        self.current_type == token_type
    }

    fn match_type(&mut self, token_type: TokenType) -> bool {
        if self.check(token_type) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume<'b>(&mut self, token_type: TokenType, msg: &'b str) -> Result<(), String> {
        if self.check(token_type) {
            self.advance();
            Ok(())
        } else {
            err!("{}", msg)
        }
    }

    fn synchronize(&mut self) {
        loop {
            match self.current_type {
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

    fn identifier_name(&self) -> Result<&str, String> {
        self.scanner.get_identifier_name(self.previous_token)
    }
}

pub fn compile(source: &str, heap: &mut Heap) -> Result<GC<Function>, String> {
    let start = Instant::now();
    let function = heap.store(Function::new(None));
    let source = Source::new(source);
    let mut compiler = Compiler::new(FunctionType::Script, function, source, heap);
    let obj = compiler.script()?;
    println!(
        "Compilation finished in {} ns.",
        Instant::now().duration_since(start).as_nanos()
    );
    match compiler.source.error_count {
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
        Source::new("");
    }

    #[test]
    fn parse_empty_string() {
        let mut source = Source::new("");
        assert!(source.match_type(TokenType::End));
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

    #[test]
    fn upvalues() {
        let test = "
        fun makeCounter() {
            var i = 0;
            fun count() {
              i = i + 1;
              print i;
            }
            return count;
        }
        var counter = makeCounter();
        counter();
        ";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }
}
