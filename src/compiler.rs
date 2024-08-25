use std::{mem, time::Instant, usize};

use crate::{
    bitarray::BitArray,
    chunk::{Chunk, Op},
    heap::{Handle, Heap, Traceable},
    object::{Function, Value},
    scanner::{Scanner, Token, TokenType},
    strings::StringHandle,
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

struct CompileData {
    enclosing: Option<Box<CompileData>>,
    function_type: FunctionType,
    function: Handle,
    locals_captured: BitArray,
    locals_initialized: BitArray,
    locals: Vec<StringHandle>,
    scopes: Vec<u8>,
    upvalues_local: BitArray,
    upvalues: Vec<u8>,
}

impl CompileData {
    fn new(function_type: FunctionType, function: Handle, this_name: StringHandle) -> Self {
        let mut initialized = BitArray::new(256);
        initialized.add(0); // first local
        Self {
            enclosing: None,
            function_type,
            function,
            locals_captured: BitArray::new(256),
            locals_initialized: initialized,
            locals: vec![this_name],
            scopes: Vec::new(),
            upvalues_local: BitArray::new(256),
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
                return if !self.locals_initialized.get(i) {
                    err!("Can't read local variable in its own initializer.")
                } else {
                    Ok(Some(i as u8))
                };
            }
        }
    }

    fn resolve_upvalue(&mut self, name: StringHandle) -> Result<Option<u8>, String> {
        if let Some(enclosing) = &mut self.enclosing {
            if let Some(index) = enclosing.resolve_local(name)? {
                enclosing.locals_captured.add(index as usize);
                return Ok(Some(self.add_upvalue(index, true)?));
            }

            if let Some(upvalue) = enclosing.resolve_upvalue(name)? {
                return Ok(Some(self.add_upvalue(upvalue, false)?));
            }
        }
        Ok(None)
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<u8, String> {
        let count = self.upvalues.len();
        for i in 0..count {
            let upvalue = self.upvalues[i];
            if self.upvalues_local.get(upvalue as usize) && upvalue == index {
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
            return false;
        }
        let index = self.locals.len() - 1;
        self.locals_initialized.add(index);
        true
    }

    fn declare_variable(&mut self, name: StringHandle) -> Result<(), String> {
        if self.scopes.len() == 0 {
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
    data: Box<CompileData>,
    source: Source<'src>,
    heap: &'hp mut Heap,
    this_name: StringHandle,
    super_name: StringHandle,
}

impl<'src, 'hp> Compiler<'src, 'hp> {
    fn new(
        function_type: FunctionType,
        function: Handle,
        source: Source<'src>,
        heap: &'hp mut Heap,
    ) -> Self {
        let this_name = heap.intern_copy("this");
        let super_name = heap.intern_copy("super");
        Self {
            data: Box::from(CompileData::new(function_type, function, this_name)),
            source,
            heap,
            this_name,
            super_name,
        }
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.heap.get_mut::<Function>(self.data.function).chunk
    }

    fn emit_return(&mut self) {
        if self.data.function_type == FunctionType::Initializer {
            self.emit_byte_op(Op::GetLocal, 0);
        } else {
            self.emit_op(Op::Nil);
        }
        self.emit_op(Op::Return);
    }

    fn emit_byte_op(&mut self, op: Op, byte: u8) {
        let line = self.line_and_column().0;
        self.current_chunk().write_byte_op(op, byte, line);
    }

    fn emit_short_op(&mut self, op: Op, short: u16) {
        let line = self.line_and_column().0;
        self.current_chunk().write_short_op(op, short, line);
    }

    fn emit_invoke_op(&mut self, op: Op, constant: u8, arity: u8) {
        let line = self.line_and_column().0;
        self.current_chunk()
            .write_invoke_op(op, constant, arity, line);
    }

    fn emit_op(&mut self, op: Op) {
        let line = self.line_and_column().0;
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
        self.data.scopes.push(self.data.locals.len() as u8);
    }

    fn end_scope(&mut self) {
        let l = self.data.scopes.pop().unwrap() as usize;
        let mut index = self.data.locals.len();
        while index > l {
            index -= 1;
            self.emit_op(if self.data.locals_captured.get(index) {
                Op::CloseUpvalue
            } else {
                Op::Pop
            });
            self.data.locals.pop();
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
        match self.source.previous.0 {
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
        match self.source.scanner.get_number(self.source.previous.1) {
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
            .intern_copy(self.source.scanner.get_str(self.source.previous.1)?);
        self.emit_constant(Value::from(value))
    }

    // admit code for variable access
    fn variable(&mut self, name: StringHandle, can_assign: bool) -> Result<(), String> {
        let (arg, get, set) = {
            if let Some(arg) = self.data.resolve_local(name)? {
                (arg, Op::GetLocal, Op::SetLocal)
            } else if let Some(arg) = self.data.resolve_upvalue(name)? {
                (arg, Op::GetUpvalue, Op::SetUpvalue)
            } else {
                let arg = self.current_chunk().add_constant(Value::from(name))?;
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
        if !self.source.has_super.get(self.source.class_depth as usize) {
            return err!("Can't use 'super' in a class with no superclass.");
        }
        self.source
            .consume(TokenType::Dot, "Expect '.' after 'super'.")?;
        let index = self.identifier_constant("Expect superclass method name.")?;
        self.variable(self.this_name, false)?;
        if self.source.match_type(TokenType::LeftParen) {
            let arity = self.argument_list()?;
            self.variable(self.super_name, false)?;
            self.emit_invoke_op(Op::SuperInvoke, index, arity);
        } else {
            self.variable(self.super_name, false)?;
            self.emit_byte_op(Op::GetSuper, index);
        }
        Ok(())
    }

    fn this(&mut self, can_assign: bool) -> Result<(), String> {
        if self.source.class_depth == 0 {
            return err!("Can't use 'this' outside of a class.");
        }
        self.variable(self.this_name, can_assign)
    }

    fn unary(&mut self) -> Result<(), String> {
        self.parse_precedence(Prec::Unary)?;
        match self.source.previous.0 {
            TokenType::Bang => self.emit_op(Op::Not),
            TokenType::Minus => self.emit_op(Op::Negative),
            _ => panic!(),
        }
        Ok(())
    }

    fn parse_infix(&mut self, can_assign: bool) -> Result<(), String> {
        match self.source.previous.0 {
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
        let str = self.source.identifier_name()?;
        Ok(self.heap.intern_copy(str))
    }

    fn parse_prefix(&mut self, can_assign: bool) -> Result<(), String> {
        match self.source.previous.0 {
            TokenType::LeftParen => self.grouping(),
            TokenType::Minus | TokenType::Bang => self.unary(),
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
            _ => err!("Expect expression, found {:?}.", self.source.previous.0),
        }
    }

    fn parse_precedence(&mut self, precedence: Prec) -> Result<(), String> {
        self.source.advance();
        let can_assign = precedence <= Prec::Assignment;
        self.parse_prefix(can_assign)?;

        while precedence <= self.source.current.0.precedence() {
            self.source.advance();
            self.parse_infix(can_assign)?;
        }

        if can_assign && self.source.match_type(TokenType::Equal) {
            err!("Invalid assignment target.")
        } else {
            Ok(())
        }
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        let name = self.store_identifier()?;
        self.data.declare_variable(name)?;
        if self.data.scopes.len() > 0 {
            Ok(0)
        } else {
            self.intern(name)
        }
    }

    fn define_variable(&mut self, global: u8) {
        if !self.data.mark_initialized() {
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
                if self.heap.get_ref::<Function>(self.data.function).arity == u8::MAX {
                    return err!("Can't have more than 255 parameters.");
                }
                self.heap.get_mut::<Function>(self.data.function).arity += 1;
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

    fn line_and_column(&self) -> (u16, u16) {
        self.source.scanner.line_and_column(self.source.previous.1)
    }

    fn function(&mut self, function_type: FunctionType) -> Result<(), String> {
        let name = self.source.identifier_name()?;
        let name = self.heap.intern_copy(name);
        let function = self.heap.put(Function::new());
        self.heap.get_mut::<Function>(function).name = Some(name);
        let before = self.heap.get_ref::<Function>(function).byte_count();
        // do the head of the linked list thing
        let enclosing = mem::replace(
            &mut self.data,
            Box::from(CompileData::new(function_type, function, self.this_name)),
        );
        self.data.enclosing = Some(enclosing);

        // the recursive call
        self.function_body()?;

        // another trick
        let enclosing = self.data.enclosing.take().unwrap();
        let enclosed = mem::replace(&mut self.data, enclosing);

        self.heap.get_mut::<Function>(function).upvalue_count = enclosed.upvalues.len() as u8;
        self.heap
            .increase_byte_count(self.heap.get_ref::<Function>(function).byte_count() - before);
        let index = self.current_chunk().add_constant(Value::from(function))?;
        self.emit_byte_op(Op::Closure, index);
        let line = self.line_and_column().0;
        // notice the inefficient encoding. o/c the vm would have to use the bitarrays as well.
        for upvalue in enclosed.upvalues {
            self.current_chunk().write(
                &[enclosed.upvalues_local.get(upvalue as usize) as u8, upvalue],
                line,
            );
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
        let class_name = self.store_identifier()?;
        self.data.declare_variable(class_name)?;
        let index = self.intern(class_name)?;
        self.emit_byte_op(Op::Class, index);
        self.define_variable(index);

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
            self.data.add_local(self.super_name)?;
            self.define_variable(0);
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

        if self.source.has_super.get(self.source.class_depth as usize) {
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
        self.data.mark_initialized();
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
        if self.data.function_type == FunctionType::Script {
            return err!("Can't return from top-level code.");
        }

        if self.source.match_type(TokenType::Semicolon) {
            self.emit_return();
            Ok(())
        } else {
            if self.data.function_type == FunctionType::Initializer {
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

    fn intern(&mut self, loxtr: StringHandle) -> Result<u8, String> {
        self.current_chunk().add_constant(Value::from(loxtr))
    }

    fn identifier_constant(&mut self, error_msg: &str) -> Result<u8, String> {
        self.source.consume(TokenType::Identifier, error_msg)?;
        let name = self.store_identifier()?;
        self.intern(name)
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
            let (l, c) = self.line_and_column();
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

    fn script(&mut self) -> Result<Handle, String> {
        let before = self
            .heap
            .get_ref::<Function>(self.data.function)
            .byte_count();
        while !self.source.match_type(TokenType::End) {
            self.declaration();
        }
        self.emit_return();
        self.heap.increase_byte_count(
            self.heap
                .get_ref::<Function>(self.data.function)
                .byte_count()
                - before,
        );
        Ok(self.data.function)
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
    current: Token,
    previous: Token,
    has_super: BitArray,
    class_depth: u8,
    // status
    error_count: u8,
}

impl<'src> Source<'src> {
    pub fn new(source: &'src str) -> Self {
        let mut scanner = Scanner::new(source);
        let current = scanner.next();
        Self {
            scanner,
            current,
            previous: Token(TokenType::Begin, usize::MAX),
            has_super: BitArray::new(256),
            class_depth: 0,
            error_count: 0,
        }
    }

    fn advance(&mut self) {
        self.previous = self.current;
        self.current = self.scanner.next();
    }

    fn check(&self, token_type: TokenType) -> bool {
        self.current.0 == token_type
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
            match self.current.0 {
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
        self.scanner.get_identifier_name(self.previous.1)
    }
}

pub fn compile(source: &str, heap: &mut Heap) -> Result<Handle, String> {
    let start = Instant::now();
    let function = heap.put(Function::new());
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
        let result = compile("", &mut Heap::new(0));
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
        let result = compile(test, &mut Heap::new(0));
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
        let result = compile(test, &mut Heap::new(0));
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
        let result = compile(test, &mut Heap::new(0));
        assert!(result.is_ok());
    }

    #[test]
    fn disassemble() {
        let test = "var a = 1;
        var b = 2;
        print a + b;";
        let mut heap = Heap::new(0);
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn printing() {
        let test = "print \"hi\"; // \"hi\".";
        let mut heap = Heap::new(0);
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn boolean_logic() {
        let test = "print \"hi\" or 2; // \"hi\".";
        let mut heap = Heap::new(0);
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
        let mut heap = Heap::new(0);
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
        let mut heap = Heap::new(0);
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }

    #[test]
    fn identity_function() {
        let test = "fun id(x) { return x; }";
        let mut heap = Heap::new(0);
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
        let mut heap = Heap::new(0);
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
        let mut heap = Heap::new(0);
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
        let mut heap = Heap::new(0);
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
        let mut heap = Heap::new(0);
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
        let mut heap = Heap::new(0);
        let result = compile(test, &mut heap);
        assert!(result.is_ok(), "{}", result.unwrap_err());
        disassemble!(&result.unwrap().chunk);
    }
}
