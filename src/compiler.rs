use std::{mem, time::Instant};

use crate::{
    bitarray::BitArray,
    functions::{Chunk, ChunkFrame, FunctionHandle},
    heap::Heap,
    op::Op,
    scanner::{Scanner, TokenType, Tokens},
    strings::StringHandle,
    values::Value,
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum FunctionType {
    Function,
    Initializer,
    Method,
    Script,
}

struct CompileBuffer {
    code: Vec<u8>,
    lines: Vec<u16>,
    run_lengths: Vec<u16>,
    constants: Vec<Value>,
    frames: Vec<ChunkFrame>,
}

impl CompileBuffer {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            lines: Vec::new(),
            run_lengths: Vec::new(),
            constants: Vec::new(),
            frames: Vec::new(),
        }
    }

    fn put_line(&mut self, line: u16, run_length: u16) {
        if self.lines.len() > 0 {
            let index = self.lines.len() - 1;
            if self.lines[index] == line {
                self.run_lengths[index] += run_length;
                return;
            }
        }
        self.lines.push(line);
        self.run_lengths.push(run_length);
    }

    pub fn write(&mut self, bytes: &[u8], line: u16) {
        self.code.extend_from_slice(bytes);
        self.put_line(line, bytes.len() as u16);
    }

    pub fn patch_jump(&mut self, offset: usize) -> Result<(), String> {
        assert!({
            let op = self.code[offset - 1];
            op == (Op::Jump as u8) || op == (Op::JumpIfFalse as u8) || op == (Op::Loop as u8)
        });
        let jump = self.code.len() - offset;
        if jump > u16::MAX as usize {
            return err!("Jump too large");
        }
        if jump == 0 {
            return err!("Not a jump");
        }
        self.code[offset] = (jump >> 8) as u8;
        self.code[offset + 1] = jump as u8;
        Ok(())
    }

    pub fn ip(&self) -> usize {
        self.code.len()
    }

    // mind the offset!
    fn add_constant(&mut self, value: Value) -> Result<(), String> {
        // should this be faster?
        let offset = if let Some(frame) = self.frames.last() {
            frame.cp
        } else {
            0
        };
        let length = self.constants.len();
        for i in offset..length {
            if self.constants[i] == value {
                self.code.push((i - offset) as u8);
                return Ok(());
            }
        }
        if length - offset > u8::MAX as usize {
            return err!("Too many constants in function");
        }
        self.constants.push(value);
        self.code.push((length - offset) as u8);
        Ok(())
    }

    pub fn write_constant_op(&mut self, op: Op, constant: Value, line: u16) -> Result<(), String> {
        self.code.push(op as u8);
        self.add_constant(constant)?;
        self.put_line(line, 2);
        Ok(())
    }

    pub fn write_byte_op(&mut self, op: Op, byte: u8, line: u16) {
        self.code.push(op as u8);
        self.code.push(byte);
        self.put_line(line, 2);
    }

    pub fn write_invoke_op(
        &mut self,
        op: Op,
        constant: Value,
        arity: u8,
        line: u16,
    ) -> Result<(), String> {
        self.code.push(op as u8);
        self.add_constant(constant)?;
        self.code.push(arity);
        self.put_line(line, 3);
        Ok(())
    }
    pub fn write_short_op(&mut self, op: Op, short: u16, line: u16) {
        self.code.push(op as u8);
        self.code.push((short >> 8) as u8);
        self.code.push(short as u8);
        self.put_line(line, 3);
    }

    pub fn open_frame(&mut self) {
        self.frames.push(ChunkFrame {
            ip: self.code.len(),
            lp: self.lines.len(),
            cp: self.constants.len(),
        });
        // create a break in the run length encoding, just in case
        self.lines.push(*self.lines.last().unwrap_or(&0));
        self.run_lengths.push(0);
    }

    pub fn close_frame(&mut self, chunk: &mut Chunk) -> usize {
        let ChunkFrame { ip, lp, cp } = self.frames.pop().unwrap_or(ChunkFrame {
            ip: 0,
            lp: 0,
            cp: 0,
        });
        let frame = chunk.add(
            &self.code[ip..],
            &self.lines[lp..],
            &self.run_lengths[lp..],
            &self.constants[cp..],
        );
        self.code.truncate(ip);
        self.lines.truncate(lp);
        self.run_lengths.truncate(lp);
        self.constants.truncate(cp);
        frame
    }
}

struct CompileData {
    function_type: FunctionType,
    locals_captured: BitArray,
    locals_initialized: BitArray,
    locals: Vec<StringHandle>,
    scopes: Vec<u8>,
    upvalues_local: BitArray,
    upvalues: Vec<u8>,
}

impl CompileData {
    fn new(function_type: FunctionType, this_name: StringHandle) -> Self {
        let mut initialized = BitArray::new();
        initialized.add(0); // first local
        Self {
            function_type,
            locals_captured: BitArray::new(),
            locals_initialized: initialized,
            locals: vec![this_name],
            scopes: Vec::new(),
            upvalues_local: BitArray::new(),
            upvalues: Vec::new(),
        }
    }

    fn resolve_local(&self, name: StringHandle) -> Result<Option<u8>, String> {
        let mut i = self.locals.len();
        loop {
            if i == 0 {
                return Ok(None);
            } else {
                i -= 1;
            }
            if self.locals[i] == name {
                return if !self.locals_initialized.has(i) {
                    err!("Can't read local variable in its own initializer.")
                } else {
                    Ok(Some(i as u8))
                };
            }
        }
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<u8, String> {
        let count = self.upvalues.len();
        for i in 0..count {
            let upvalue = self.upvalues[i];
            if upvalue == index && self.upvalues_local.has(upvalue as usize) == is_local {
                return Ok(i as u8);
            }
        }
        if count > u8::MAX as usize {
            return err!("Too many closure variables in function.");
        }
        self.upvalues.push(index);
        if is_local {
            self.upvalues_local.add(index as usize);
        }
        Ok(count as u8)
    }

    fn add_local(&mut self, name: StringHandle) -> Result<(), String> {
        if self.locals.len() > u8::MAX as usize {
            return err!("Too many local variables in function.");
        }
        self.locals.push(name);
        Ok(())
    }

    fn mark_initialized(&mut self) -> bool {
        if self.scopes.len() == 0 {
            // global scope, so initialization is not needed
            return false;
        }
        self.locals_initialized.add(self.locals.len() - 1);
        true
    }

    fn declare_variable(&mut self, name: StringHandle) -> Result<(), String> {
        if self.scopes.len() == 0 {
            // global scope, nothing to declare
            return Ok(());
        }
        let l = self.scopes[self.scopes.len() - 1] as usize;
        let mut i = self.locals.len();
        while i > l {
            i -= 1;
            if self.locals[i] == name {
                return Err(format!("Already a variable with this name in this scope."));
            }
        }
        self.add_local(name)
    }
}

struct Compiler<'src, 'hp> {
    head: CompileData,
    tail: Vec<CompileData>,
    buffer: CompileBuffer,
    source: Source<'src>,
    heap: &'hp mut Heap,
    this_name: StringHandle,
    super_name: StringHandle,
}

impl<'src, 'hp> Compiler<'src, 'hp> {
    fn new(function_type: FunctionType, source: Source<'src>, heap: &'hp mut Heap) -> Self {
        let this_name = heap.strings.put("this");
        let super_name = heap.strings.put("super");
        Self {
            head: CompileData::new(function_type, this_name),
            tail: Vec::new(),
            buffer: CompileBuffer::new(),
            source,
            heap,
            this_name,
            super_name,
        }
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
        let line = self.source.previous_line();
        self.buffer.write_byte_op(op, byte, line);
    }

    fn emit_short_op(&mut self, op: Op, short: u16) {
        let line = self.source.previous_line();
        self.buffer.write_short_op(op, short, line);
    }

    fn emit_invoke_op(&mut self, op: Op, constant: Value, arity: u8) -> Result<(), String> {
        let line = self.source.previous_line();
        self.buffer.write_invoke_op(op, constant, arity, line)
    }

    fn emit_op(&mut self, op: Op) {
        let line = self.source.previous_line();
        self.buffer.write(&[op as u8], line);
    }

    fn emit_loop(&mut self, start: usize) -> Result<(), String> {
        let offset = self.buffer.ip() - start + 1;
        if offset > u16::MAX as usize {
            err!("loop size to large")
        } else {
            self.emit_short_op(Op::Loop, offset as u16);
            Ok(())
        }
    }

    fn emit_jump(&mut self, instruction: Op) -> usize {
        self.emit_short_op(instruction, 0xffff);
        self.buffer.ip() - 2
    }

    fn emit_constant_op(&mut self, op: Op, value: Value) -> Result<(), String> {
        let line = self.source.previous_line();
        self.buffer.write_constant_op(op, value, line)
    }

    fn begin_scope(&mut self) {
        let scope_depth = self.head.locals.len() as u8;
        self.head.scopes.push(scope_depth);
    }

    fn end_scope(&mut self) {
        let l = self.head.scopes.pop().unwrap() as usize;
        let mut index = self.head.locals.len();
        while index > l {
            index -= 1;
            self.emit_op(if self.head.locals_captured.has(index) {
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

        self.buffer.patch_jump(end_jump)
    }

    fn binary(&mut self) -> Result<(), String> {
        match self.source.previous_type() {
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
            self.emit_constant_op(Op::SetProperty, index)
        } else if self.source.match_type(TokenType::LeftParen) {
            let arity = self.argument_list()?;
            self.emit_invoke_op(Op::Invoke, index, arity)
        } else {
            self.emit_constant_op(Op::GetProperty, index)
        }
    }

    fn number(&mut self) -> Result<(), String> {
        match Scanner::get_number(self.source.source, self.source.previous_offset()) {
            Ok(number) => self.emit_constant_op(Op::Constant, Value::from(number)),
            Err(err) => Err(err.to_string()),
        }
    }

    fn or(&mut self) -> Result<(), String> {
        let else_jump = self.emit_jump(Op::JumpIfFalse);
        let end_jump = self.emit_jump(Op::Jump);

        self.buffer.patch_jump(else_jump)?;
        self.emit_op(Op::Pop);

        self.parse_precedence(Prec::Or)?;

        self.buffer.patch_jump(end_jump)?;
        Ok(())
    }

    fn string(&mut self) -> Result<(), String> {
        let name = Scanner::get_str(self.source.source, self.source.previous_offset())?;
        let value = self.heap.strings.put(name);
        self.emit_constant_op(Op::Constant, Value::from(value))
    }

    // this says something about resolve upvalue...
    fn data(&mut self, i: usize) -> &mut CompileData {
        if i == self.tail.len() {
            &mut self.head
        } else {
            &mut self.tail[i]
        }
    }

    fn resolve_upvalue(&mut self, i: usize, name: StringHandle) -> Result<Option<u8>, String> {
        if i == 0 {
            return Ok(None);
        }
        if let Some(index) = self.data(i - 1).resolve_local(name)? {
            self.data(i - 1).locals_captured.add(index as usize);
            return Ok(Some(self.data(i).add_upvalue(index, true)?));
        }
        if let Some(upvalue) = self.resolve_upvalue(i - 1, name)? {
            return Ok(Some(self.data(i).add_upvalue(upvalue, false)?));
        }
        return Ok(None);
    }

    // emit code for variable access
    fn variable(&mut self, name: StringHandle, can_assign: bool) -> Result<(), String> {
        let is_assignment = can_assign && self.source.match_type(TokenType::Equal);
        if is_assignment {
            self.expression()?;
        }
        if let Some(arg) = self.head.resolve_local(name)? {
            self.emit_byte_op(
                if is_assignment {
                    Op::SetLocal
                } else {
                    Op::GetLocal
                },
                arg,
            );
            return Ok(());
        }
        if let Some(arg) = self.resolve_upvalue(self.tail.len(), name)? {
            self.emit_byte_op(
                if is_assignment {
                    Op::SetUpvalue
                } else {
                    Op::GetUpvalue
                },
                arg,
            );
            return Ok(());
        }
        self.emit_constant_op(
            if is_assignment {
                Op::SetGlobal
            } else {
                Op::GetGlobal
            },
            Value::from(name),
        )
    }

    fn super_(&mut self) -> Result<(), String> {
        if self.source.class_depth == 0 {
            return err!("Can't use 'super' outside of a class.");
        }
        if !self.source.has_super.has(self.source.class_depth as usize) {
            return err!("Can't use 'super' in a class with no superclass.");
        }
        self.source
            .consume(TokenType::Dot, "Expect '.' after 'super'.")?;
        let index = self.identifier_constant("Expect superclass method name.")?;
        self.variable(self.this_name, false)?;
        if self.source.match_type(TokenType::LeftParen) {
            let arity = self.argument_list()?;
            self.variable(self.super_name, false)?;
            self.emit_invoke_op(Op::SuperInvoke, index, arity)?;
        } else {
            self.variable(self.super_name, false)?;
            self.emit_constant_op(Op::GetSuper, index)?;
        }
        Ok(())
    }

    fn this(&mut self, can_assign: bool) -> Result<(), String> {
        if self.source.class_depth == 0 {
            return err!("Can't use 'this' outside of a class.");
        }
        self.variable(self.this_name, can_assign)
    }

    fn parse_infix(&mut self, can_assign: bool) -> Result<(), String> {
        match self.source.previous_type() {
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

    fn store_identifier(&mut self) -> Result<StringHandle, String> {
        let str = Scanner::get_identifier_name(self.source.source, self.source.previous_offset())?;
        Ok(self.heap.strings.put(str))
    }

    fn parse_prefix(&mut self, can_assign: bool) -> Result<(), String> {
        match self.source.previous_type() {
            TokenType::LeftParen => self.grouping(),
            TokenType::Minus => {
                self.parse_precedence(Prec::Unary)?;
                self.emit_op(Op::Negative);
                Ok(())
            }
            TokenType::Bang => {
                self.parse_precedence(Prec::Unary)?;
                self.emit_op(Op::Not);
                Ok(())
            }
            TokenType::Identifier => {
                let name = self.store_identifier()?;
                self.variable(name, can_assign)
            }
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
            _ => err!(
                "Expect expression, found {:?}.",
                self.source.previous_type()
            ),
        }
    }

    fn parse_precedence(&mut self, precedence: Prec) -> Result<(), String> {
        self.source.advance();
        let can_assign = precedence <= Prec::Assignment;
        self.parse_prefix(can_assign)?;

        while precedence <= self.source.current_type().precedence() {
            self.source.advance();
            self.parse_infix(can_assign)?;
        }

        if can_assign && self.source.match_type(TokenType::Equal) {
            err!("Invalid assignment target.")
        } else {
            Ok(())
        }
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<Option<StringHandle>, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        let name: StringHandle = self.store_identifier()?;
        self.head.declare_variable(name)?;
        Ok(if self.head.scopes.len() > 0 {
            None
        } else {
            // global
            Some(name)
        })
    }

    fn expression(&mut self) -> Result<(), String> {
        self.parse_precedence(Prec::Assignment)
    }

    fn grouping(&mut self) -> Result<(), String> {
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after expression.")
    }

    fn function_body(&mut self) -> Result<u8, String> {
        self.begin_scope();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after function name.")?;
        let mut arity: u8 = 0;
        if !self.source.check(TokenType::RightParen) {
            loop {
                if arity == u8::MAX {
                    return err!("Can't have more than 255 parameters.");
                }
                arity += 1;
                let index = self.parse_variable("Expect parameter name")?;
                self.define_variable(index)?;
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
        Ok(arity)
    }

    fn define_variable(&mut self, index: Option<StringHandle>) -> Result<(), String> {
        Ok(if let Some(name) = index {
            self.emit_constant_op(Op::DefineGlobal, Value::from(name))?;
        } else {
            self.head.mark_initialized();
        })
    }

    fn function(&mut self, function_type: FunctionType) -> Result<(), String> {
        let name = Scanner::get_identifier_name(self.source.source, self.source.previous_offset())?;
        let name = self.heap.strings.put(name);

        self.tail.push(mem::replace(
            &mut self.head,
            CompileData::new(function_type, self.this_name),
        ));
        self.buffer.open_frame();
        // the 'recursive' call
        let arity = self.function_body()?;

        let enclosed = mem::replace(&mut self.head, self.tail.pop().unwrap());

        // careful: this only works because of the function up there.
        let frame = self.buffer.close_frame(&mut self.heap.functions.chunk);
        // function creation on the heap
        let function = self.heap.functions.new_function(
            Some(name),
            arity,
            enclosed.upvalues.len() as u8,
            frame,
        );

        self.emit_constant_op(Op::Closure, Value::from(function))?;

        let line = self.source.previous_line();
        // notice the inefficient encoding. o/c the vm would have to use the bitarrays as well.
        for upvalue in enclosed.upvalues {
            self.buffer.write(
                &[enclosed.upvalues_local.has(upvalue as usize) as u8, upvalue],
                line,
            );
        }
        Ok(())
    }

    fn method(&mut self) -> Result<(), String> {
        self.source
            .consume(TokenType::Identifier, "Expect method name.")?;
        let name = Scanner::get_identifier_name(self.source.source, self.source.previous_offset())?;
        let function_type = if name == "init" {
            FunctionType::Initializer
        } else {
            FunctionType::Method
        };
        let loxtr = self.heap.strings.put(name);
        self.function(function_type)?;
        self.emit_constant_op(Op::Method, Value::from(loxtr))?;
        Ok(())
    }

    fn class(&mut self) -> Result<(), String> {
        self.source
            .consume(TokenType::Identifier, "Expect class name.")?;
        let class_name = self.store_identifier()?;
        self.head.declare_variable(class_name)?;
        self.emit_constant_op(Op::Class, Value::from(class_name))?;
        self.define_variable(if self.head.scopes.len() == 0 {
            Some(class_name)
        } else {
            None
        })?;

        self.head.mark_initialized();

        if self.source.class_depth == 127 {
            return err!("Cannot nest classes that deep");
        }
        self.source.class_depth += 1;

        // super decl
        if self.source.match_type(TokenType::Less) {
            self.source
                .consume(TokenType::Identifier, "Expect superclass name.")?;
            let super_name = self.store_identifier()?;
            self.variable(super_name, false)?;
            if class_name == super_name {
                return err!("A class can't inherit from itself.");
            }
            self.begin_scope();
            // yes, rust asks for this
            let name = self.super_name;
            self.head.add_local(name)?;
            self.head.mark_initialized();
            self.variable(class_name, false)?;
            self.emit_op(Op::Inherit);
            self.source.has_super.add(self.source.class_depth as usize);
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
                return err!("Expect '}}' after class body.");
            }
            self.method()?;
        }
        self.emit_op(Op::Pop);

        if self.source.has_super.has(self.source.class_depth as usize) {
            self.end_scope();
        }

        self.source
            .has_super
            .remove(self.source.class_depth as usize);
        self.source.class_depth -= 1;
        Ok(())
    }

    fn fun_declaration(&mut self) -> Result<(), String> {
        let index = self.parse_variable("Expect function name.")?;
        self.head.mark_initialized();
        self.function(FunctionType::Function)?;
        if let Some(name) = index {
            self.emit_constant_op(Op::DefineGlobal, Value::from(name))?;
        }
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
        self.define_variable(index)
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
        let mut loop_start = self.buffer.ip();
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
            let increment_start = self.buffer.ip();
            self.expression()?;
            self.emit_op(Op::Pop);
            self.source
                .consume(TokenType::RightParen, "Expect ')' after for clauses.")?;

            self.emit_loop(loop_start)?;
            loop_start = increment_start;

            self.buffer.patch_jump(body_jump)?;
        }

        self.statement()?;
        self.emit_loop(loop_start)?;
        if let Some(i) = exit_jump {
            self.buffer.patch_jump(i)?;
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

        self.buffer.patch_jump(then_jump)?;
        self.emit_op(Op::Pop);
        if self.source.match_type(TokenType::Else) {
            self.statement()?;
        }

        self.buffer.patch_jump(else_jump)?;
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
        let loop_start = self.buffer.ip();
        self.source
            .consume(TokenType::LeftParen, "Expect '(' after 'while'.")?;
        self.expression()?;
        self.source
            .consume(TokenType::RightParen, "Expect ')' after condition.")?;

        let exit_jump = self.emit_jump(Op::JumpIfFalse);
        self.emit_op(Op::Pop);
        self.statement()?;
        self.emit_loop(loop_start)?;

        self.buffer.patch_jump(exit_jump)?;
        self.emit_op(Op::Pop);
        Ok(())
    }

    fn identifier_constant(&mut self, error_msg: &str) -> Result<Value, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        Ok(Value::from(self.store_identifier()?))
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
            let (l, c) =
                Scanner::line_and_column(self.source.source, self.source.previous_offset());
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

    fn script(mut self) -> Result<FunctionHandle, String> {
        while !self.source.match_type(TokenType::End) {
            self.declaration();
        }
        self.emit_return();
        match self.source.error_count {
            0 => (),
            1 => return err!("There was a compile time error."),
            more => return err!("There were {} compile time errors.", more),
        }

        let frame = self.buffer.close_frame(&mut self.heap.functions.chunk);
        let fh = self.heap.functions.new_function(None, 0, 0, frame);
        assert!(self.buffer.frames.is_empty());
        Ok(fh)
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
    source: &'src str,
    tokens: Tokens,
    lines: Vec<u16>,
    current: usize,
    has_super: BitArray,
    class_depth: u8,
    // status
    error_count: u8,
}

impl<'src> Source<'src> {
    pub fn new(source: &'src str) -> Self {
        let tokens = Scanner::scan(source);
        let lines = Source::get_line_numbers(source, &tokens.froms);
        Self {
            source,
            tokens,
            lines,
            current: 0,
            has_super: BitArray::new(),
            class_depth: 0,
            error_count: 0,
        }
    }

    fn get_line_numbers(source: &'src str, offsets: &Vec<usize>) -> Vec<u16> {
        let new_lines = Scanner::new_lines(source);
        let mut line_numbers = Vec::new();
        let mut count = 0;
        for &offset in offsets {
            if count < new_lines.len() && new_lines[count] < offset {
                count += 1;
            }
            line_numbers.push(count as u16 + 1);
        }
        line_numbers
    }

    fn advance(&mut self) {
        self.current += 1;
    }

    fn check(&self, token_type: TokenType) -> bool {
        self.tokens.types[self.current] == token_type
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

    fn current_type(&self) -> TokenType {
        self.tokens.types[self.current]
    }

    fn previous_type(&self) -> TokenType {
        self.tokens.types[self.current - 1]
    }

    fn previous_offset(&self) -> usize {
        self.tokens.froms[self.current - 1]
    }

    fn previous_line(&self) -> u16 {
        self.lines[self.current - 1]
    }

    fn synchronize(&mut self) {
        loop {
            match self.current_type() {
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

pub fn compile(source: &str, heap: &mut Heap) -> Result<FunctionHandle, String> {
    let start = Instant::now();
    let source = Source::new(source);
    let compiler = Compiler::new(FunctionType::Script, source, heap);
    let fh = compiler.script()?;
    println!(
        "Compilation finished in {} ns.",
        Instant::now().duration_since(start).as_nanos(),
    );
    Ok(fh)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! disassemble {
        ($bc:expr,$h:expr) => {
            #[cfg(feature = "trace")]
            {
                use crate::debug::Disassembler;
                Disassembler::disassemble($bc, $h);
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
        disassemble!(&result.unwrap(), &heap);
    }

    #[test]
    fn printing() {
        let test = "print \"hi\"; // \"hi\".";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap(), &heap);
    }

    #[test]
    fn boolean_logic() {
        let test = "print \"hi\" or 2; // \"hi\".";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap(), &heap);
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
        disassemble!(&result.unwrap(), &heap);
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
        disassemble!(&result.unwrap(), &heap);
    }

    #[test]
    fn identity_function() {
        let test = "fun id(x) { return x; }";
        let mut heap = Heap::new();
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap(), &heap);
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
        disassemble!(&result.unwrap(), &heap);
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
        disassemble!(&result.unwrap(), &heap);
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
        disassemble!(&result.unwrap(), &heap);
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
        disassemble!(&result.unwrap(), &heap);
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
        disassemble!(&result.unwrap(), &heap);
    }
}
