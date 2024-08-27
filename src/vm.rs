use std::time;

use crate::{
    byte_code::{ByteCode, FunctionHandle},
    call_stack::CallStack,
    chunk::Op,
    common::U8_COUNT,
    compiler::compile,
    heap::{Handle, Heap, Kind, Traceable},
    natives::Natives,
    object::{BoundMethod, Class, Closure, Instance, Upvalue, Value},
    strings::{Map, StringHandle},
};

const MAX_FRAMES: usize = 64; // > 0, < 2^16 - 1
const STACK_SIZE: usize = (MAX_FRAMES as usize) * U8_COUNT;

fn clock_native(_args: &[Value]) -> Result<Value, String> {
    match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
        Ok(duration) => Ok(Value::from(duration.as_secs_f64())),
        Err(x) => Err(x.to_string()),
    }
}

macro_rules! binary_op {
    ($self:ident, $a:ident, $b:ident, $value:expr) => {{
        if let &[Value::Number($a), Value::Number($b)] = $self.tail(2)? {
            $self.stack_top -= 2;
            $self.push(Value::from($value));
        } else {
            return err!("Operands must be numbers.");
        }
    }};
}

pub struct VM {
    values: [Value; STACK_SIZE],
    stack_top: usize,
    call_stack: CallStack<MAX_FRAMES>,
    open_upvalues: Option<Handle>,
    globals: Map<Value>,
    init_string: StringHandle,
    heap: Heap,
    byte_code: ByteCode,
    natives: Natives,
}

impl VM {
    pub fn new(mut heap: Heap, byte_code: ByteCode) -> Self {
        let init_string = heap.intern_copy("init");
        let mut s = Self {
            values: [Value::Nil; STACK_SIZE],
            stack_top: 0,
            call_stack: CallStack::new(),
            open_upvalues: None,
            globals: Map::new(),
            init_string,
            heap,
            byte_code,
            natives: Natives::new(),
        };
        s.define_native("clock", clock_native);
        s
    }
    pub fn capture_upvalue(&mut self, location: usize) -> Handle {
        let mut previous = None;
        let mut current = self.open_upvalues;
        while let Some(link) = current {
            if let &Upvalue::Open(index, next) = self.heap.get_ref::<Upvalue>(link) {
                if location == index {
                    return link;
                }
                if location < index {
                    break;
                }
                previous = current;
                current = next
            } else {
                break;
            }
        }
        let created = self.new_obj(Upvalue::Open(location, current));
        match previous {
            None => {
                self.open_upvalues = Some(created);
            }
            Some(obj) => {
                if let &Upvalue::Open(x, _) = self.heap.get_ref::<Upvalue>(obj) {
                    *self.heap.get_mut::<Upvalue>(obj) = Upvalue::Open(x, Some(created))
                }
            }
        }
        created
    }

    fn close_upvalues(&mut self, location: usize) {
        while let Some(link) = self.open_upvalues {
            if let &Upvalue::Open(l, next) = *&self.heap.get_ref::<Upvalue>(link) {
                if l < location {
                    return;
                }
                *self.heap.get_mut::<Upvalue>(link) = Upvalue::Closed(self.values[l]);
                self.open_upvalues = next;
            } else {
                self.open_upvalues = None;
            }
        }
    }

    fn new_obj<T: Traceable>(&mut self, t: T) -> Handle {
        if self.heap.needs_gc() {
            let (roots, keyset) = self.roots();
            self.heap.retain(roots, keyset);
        }
        self.heap.put(t)
    }

    fn roots(&mut self) -> (Vec<Handle>, Vec<StringHandle>) {
        let mut collector = Vec::new();
        let mut strings = Vec::new();
        #[cfg(feature = "log_gc")]
        {
            println!("collect stack objects");
        }
        for i in 0..self.stack_top {
            if let Value::Object(handle) = self.values[i] {
                collector.push(handle);
            }
        }
        #[cfg(feature = "log_gc")]
        {
            println!("collect frames");
        }
        self.call_stack.trace(&mut collector);
        #[cfg(feature = "log_gc")]
        {
            println!("collect upvalues");
        }
        if let Some(upvalue) = self.open_upvalues {
            collector.push(Handle::from(upvalue));
        }
        #[cfg(feature = "log_gc")]
        {
            println!("collect globals");
        }
        self.globals.trace(&mut collector, &mut strings);
        // no compiler roots
        #[cfg(feature = "log_gc")]
        {
            println!("collect init string");
        }
        self.byte_code.trace(&mut collector, &mut strings);
        strings.push(self.init_string);
        (collector, strings)
    }

    fn define_native(
        &mut self,
        name: &str,
        native_fn: fn(args: &[Value]) -> Result<Value, String>,
    ) {
        let key = self.heap.intern_copy(name);
        // are the protections still needed?
        self.push(Value::from(key));
        self.globals
            .set(key, Value::Native(self.natives.store(native_fn)));
        self.pop();
    }

    fn push(&mut self, value: Value) {
        self.values[self.stack_top] = value;
        self.stack_top += 1;
    }

    fn pop(&mut self) -> Value {
        self.stack_top -= 1;
        self.values[self.stack_top]
    }

    fn peek(&self, distance: usize) -> Value {
        self.values[self.stack_top - 1 - distance]
    }

    fn call(&mut self, closure: Handle, arity: u8) -> Result<(), String> {
        let handle = self.heap.get_ref::<Closure>(closure).function;
        let expected = self.byte_code.function_ref(handle).arity;
        if arity != expected {
            return err!("Expected {} arguments but got {}.", expected, arity);
        }
        self.call_stack.push(
            self.stack_top - arity as usize - 1,
            closure,
            &self.heap,
            &self.byte_code,
        )
    }

    fn call_value(&mut self, callee: Value, arity: u8) -> Result<(), String> {
        if let Value::Object(handle) = callee {
            match self.heap.kind(handle) {
                Kind::BoundMethod => {
                    let bm = self.heap.get_ref::<BoundMethod>(handle);
                    self.values[self.stack_top - arity as usize - 1] = Value::from(bm.receiver);
                    return self.call(bm.method, arity);
                }
                Kind::Class => {
                    let instance = self.new_obj(Instance::new(handle));
                    self.values[self.stack_top - arity as usize - 1] = Value::from(instance);
                    let obj = self.heap.get_ref::<Class>(handle);
                    if let Some(init) = obj.methods.get(self.init_string) {
                        return self.call(init, arity);
                    } else if arity > 0 {
                        return err!("Expected no arguments but got {}.", arity);
                    } else {
                        return Ok(());
                    }
                }
                Kind::Closure => {
                    return self.call(handle, arity);
                }
                _ => (),
            }
        }
        if let Value::Native(handle) = callee {
            let result = self.natives.call(handle, self.tail(arity as usize)?)?;
            self.stack_top -= arity as usize + 1;
            self.push(result);
            return Ok(());
        }
        err!(
            "Can only call functions and classes, not '{}'",
            callee.to_string(&self.heap, &self.byte_code)
        )
    }

    fn invoke_from_class(
        &mut self,
        class: Handle,
        name: StringHandle,
        arity: u8,
    ) -> Result<(), String> {
        match self.heap.get_ref::<Class>(class).methods.get(name) {
            None => err!("Undefined property '{}'", self.heap.get_str(name)),
            Some(method) => self.call(method, arity),
        }
    }

    fn invoke(&mut self, name: StringHandle, arity: u8) -> Result<(), String> {
        let value = self.peek(arity as usize);
        let instance = self
            .heap
            .try_ref::<Instance>(value)
            .ok_or("Only instances have methods.")?;
        if let Some(property) = instance.properties.get(name) {
            self.values[self.stack_top - arity as usize - 1] = property;
            self.call_value(property, arity)
        } else {
            self.invoke_from_class(instance.class, name, arity)
        }
    }

    fn bind_method(&mut self, class: Handle, name: StringHandle) -> Result<(), String> {
        match self.heap.get_ref::<Class>(class).methods.get(name) {
            None => err!("Undefined property '{}'.", self.heap.get_str(name)),
            Some(method) => {
                if let Value::Object(instance) = self.peek(0) {
                    let bm = self.new_obj(BoundMethod::new(instance, method));
                    self.pop();
                    self.push(Value::from(bm));
                    Ok(())
                } else {
                    err!(
                        "Cannot bind method {} to {}",
                        self.heap.to_string(method, &self.byte_code),
                        self.heap.get_str(name)
                    )
                }
            }
        }
    }

    fn define_method(&mut self, name: StringHandle) -> Result<(), String> {
        if let Ok(&[Value::Object(class), Value::Object(method)]) = self.tail(2) {
            let class: &mut Class = self.heap.get_mut(class);
            let before_count = class.byte_count();
            class.methods.set(name, method);
            let after_count = class.byte_count();
            self.heap.increase_byte_count(after_count - before_count);
            self.pop();
            Ok(())
        } else {
            err!("Method definition failed")
        }
    }

    // combined to avoid gc errors
    fn push_traceable<T: Traceable>(&mut self, traceable: T) {
        let value = self.new_obj(traceable);
        self.push(Value::from(value));
    }

    fn run(&mut self) -> Result<(), String> {
        loop {
            let instruction = Op::from(self.call_stack.read_byte(&self.byte_code));
            #[cfg(feature = "trace")]
            {
                print!("stack: ");
                for i in 0..self.stack_top {
                    print!("{};", &self.values[i]);
                }
                println!("");

                // print!("globals: ");
                // for (k, v) in &self.globals {
                //     print!("{}:{};", **k, v)
                // }
                // println!("");

                let ip = self.top_frame().ip;
                println!("ip: {}", ip);
                println!("line: {}", self.top_frame().chunk().lines[ip as usize]);
                println!("op code: {:?}", instruction);
                println!();
            }
            match instruction {
                Op::Add => {
                    if let &[a, b] = self.tail(2)? {
                        if let (Value::String(a), Value::String(b)) = (a, b) {
                            let c = self.heap.concat(a, b).ok_or("Missing strings")?;
                            self.stack_top -= 2;
                            self.push(Value::String(c));
                            continue;
                        }

                        if let (Value::Number(a), Value::Number(b)) = (a, b) {
                            self.stack_top -= 2;
                            self.push(Value::from(a + b));
                            continue;
                        }

                        return err!(
                            "Operands must be either numbers or strings, found '{}' and '{}'",
                            a.to_string(&self.heap, &self.byte_code),
                            b.to_string(&self.heap, &self.byte_code),
                        );
                    }
                }
                Op::Call => {
                    let arity = self.call_stack.read_byte(&self.byte_code);
                    self.call_value(self.peek(arity as usize), arity)?;
                }
                Op::Class => {
                    let name = self.call_stack.read_string(&self.byte_code, &self.heap)?;
                    self.push_traceable(Class::new(name));
                }
                Op::CloseUpvalue => {
                    self.close_upvalues(self.stack_top - 1);
                    self.pop();
                }
                Op::Closure => {
                    let function = self
                        .call_stack
                        .read_constant(&self.byte_code)
                        .as_function()?;
                    let mut traceable = Closure::new(function);
                    // garbage collection risks?
                    let f = self.byte_code.function_ref(function);
                    for _ in 0..f.upvalue_count {
                        let is_local = self.call_stack.read_byte(&self.byte_code);
                        let index = self.call_stack.read_byte(&self.byte_code) as usize;
                        traceable.upvalues.push(if is_local > 0 {
                            let location = self.call_stack.slot() + index;
                            self.capture_upvalue(location)
                        } else {
                            self.call_stack.upvalue(index, &self.heap)?
                        })
                    }
                    self.push_traceable(traceable);
                }
                Op::Constant => {
                    let value = self.call_stack.read_constant(&self.byte_code);
                    self.push(value)
                }
                Op::DefineGlobal => {
                    let name = self.call_stack.read_string(&self.byte_code, &self.heap)?;
                    self.globals.set(name, self.peek(0));
                    self.pop();
                }
                Op::Divide => binary_op!(self, a, b, a / b),
                Op::Equal => {
                    let a = self.pop();
                    let b = self.pop();
                    self.push(Value::from(a == b));
                }
                Op::False => self.push(Value::False),
                Op::GetGlobal => {
                    let name = self.call_stack.read_string(&self.byte_code, &self.heap)?;
                    if let Some(value) = self.globals.get(name) {
                        self.push(value);
                    } else {
                        return err!("Undefined variable '{}'.", self.heap.get_str(name));
                    }
                }
                Op::GetLocal => {
                    let index = self.call_stack.slot()
                        + self.call_stack.read_byte(&self.byte_code) as usize;
                    self.push(self.values[index])
                }
                Op::GetProperty => {
                    let value = self.peek(0);
                    let instance = self
                        .heap
                        .try_ref::<Instance>(value)
                        .ok_or(String::from("Only instances have properties."))?;
                    let name = self.call_stack.read_string(&self.byte_code, &self.heap)?;
                    if let Some(value) = instance.properties.get(name) {
                        // replace instance
                        self.values[self.stack_top - 1] = value;
                    } else {
                        self.bind_method(instance.class, name)?;
                    }
                }
                Op::GetSuper => {
                    let name = self.call_stack.read_string(&self.byte_code, &self.heap)?;
                    let super_class = self.pop().as_object()?;
                    self.bind_method(super_class, name)?;
                }
                Op::GetUpvalue => {
                    let value = match self.heap.get_ref::<Upvalue>(
                        self.call_stack.read_upvalue(&self.byte_code, &self.heap)?,
                    ) {
                        &Upvalue::Open(index, _) => self.values[index],
                        &Upvalue::Closed(value) => value,
                    };
                    self.push(value);
                }
                Op::Greater => {
                    binary_op!(self, a, b, a > b)
                }
                Op::Inherit => {
                    if let &[a, b] = self.tail(2)? {
                        let super_class = self
                            .heap
                            .try_ref::<Class>(a)
                            .ok_or(String::from("Super class must be a class."))?;
                        let sub_class = self
                            .heap
                            .try_mut::<Class>(b)
                            .ok_or(String::from("Sub class must be a class."))?;
                        let bytes_before = sub_class.byte_count();
                        sub_class.methods.set_all(&super_class.methods);
                        self.heap
                            .increase_byte_count(sub_class.byte_count() - bytes_before);
                        self.pop();
                    }
                }
                Op::Invoke => {
                    let name = self.call_stack.read_string(&self.byte_code, &self.heap)?;
                    let arity = self.call_stack.read_byte(&self.byte_code);
                    self.invoke(name, arity)?;
                }
                Op::Jump => self.call_stack.jump_forward(&self.byte_code),
                Op::JumpIfFalse => {
                    if self.peek(0).is_falsey() {
                        self.call_stack.jump_forward(&self.byte_code);
                    } else {
                        self.call_stack.skip();
                    }
                }
                Op::Less => binary_op!(self, a, b, a < b),
                Op::Loop => self.call_stack.jump_back(&self.byte_code),
                Op::Method => {
                    let name = self.call_stack.read_string(&self.byte_code, &self.heap)?;
                    self.define_method(name)?
                }
                Op::Multiply => binary_op!(self, a, b, a * b),
                Op::Negative => {
                    if let Value::Number(a) = self.peek(0) {
                        self.values[self.stack_top - 1] = Value::from(-a);
                    } else {
                        return err!("Operand must be a number.");
                    }
                }
                Op::Nil => self.push(Value::Nil),
                Op::Not => {
                    let pop = &self.pop();
                    self.push(Value::from(pop.is_falsey()));
                }
                Op::Pop => {
                    self.pop();
                }
                Op::Print => println!("{}", self.pop().to_string(&self.heap, &self.byte_code)),
                Op::Return => {
                    let result = self.pop();
                    let location = self.call_stack.slot();
                    self.close_upvalues(location);
                    self.call_stack.pop();
                    if self.call_stack.is_empty() {
                        self.pop();
                        return Ok(());
                    }
                    self.stack_top = location;
                    self.push(result);
                }
                Op::SetGlobal => {
                    let name = self.call_stack.read_string(&self.byte_code, &self.heap)?;
                    if !self.globals.set(name, self.peek(0)) {
                        self.globals.delete(name);
                        return err!("Undefined variable '{}'.", self.heap.get_str(name));
                    }
                }
                Op::SetLocal => {
                    let index = self.call_stack.read_byte(&self.byte_code) as usize;
                    self.values[self.call_stack.slot() + index] = self.peek(0);
                }
                Op::SetProperty => {
                    if let &[a, b] = self.tail(2)? {
                        let instance = self
                            .heap
                            .try_mut::<Instance>(a)
                            .ok_or(String::from("Only instances have fields."))?;
                        let before_count = instance.byte_count();
                        instance
                            .properties
                            .set(self.call_stack.read_string(&self.byte_code, &self.heap)?, b);
                        self.heap
                            .increase_byte_count(instance.byte_count() - before_count);
                        self.stack_top -= 2;
                        self.push(b);
                    }
                }
                Op::SetUpvalue => {
                    let upvalue = self.call_stack.read_upvalue(&self.byte_code, &self.heap)?;
                    match self.heap.get_ref(upvalue) {
                        &Upvalue::Closed(_) => {
                            *self.heap.get_mut(upvalue) = Upvalue::Closed(self.peek(0))
                        }
                        &Upvalue::Open(index, _) => self.values[index] = self.peek(0),
                    }
                }
                Op::Subtract => binary_op!(self, a, b, a - b),
                Op::SuperInvoke => {
                    let name = self.call_stack.read_string(&self.byte_code, &self.heap)?;
                    let arity = self.call_stack.read_byte(&self.byte_code);
                    let super_class = self.pop().as_object()?;
                    self.invoke_from_class(super_class, name, arity)?;
                }
                Op::True => self.push(Value::True),
            }
        }
    }

    fn tail(&self, n: usize) -> Result<&[Value], String> {
        if n <= self.stack_top {
            Ok(&self.values[self.stack_top - n..self.stack_top])
        } else {
            err!("Stack underflow")
        }
    }

    fn reset_stack(&mut self) {
        self.stack_top = 0;
        self.open_upvalues = None;
    }

    pub fn interpret(&mut self, source: &str) -> Result<(), String> {
        self.byte_code = compile(source, &mut self.heap)?;
        let closure = self.new_obj(Closure::new(FunctionHandle::MAIN));
        self.push(Value::from(closure));
        self.call(closure, 0)?;
        if let Err(msg) = self.run() {
            eprintln!("Error: {}", msg);
            // where is the stack trace!?
            self.call_stack.print_stack_trace(&self.byte_code,&self.heap);
            self.reset_stack();
            err!("Runtime error!")
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_error_on_init() {
        VM::new(Heap::new(1 << 8), ByteCode::new());
    }

    #[test]
    fn interpret_empty_string() {
        let mut vm = VM::new(Heap::new(1 << 8), ByteCode::new());
        assert!(vm.interpret("").is_ok())
    }

    #[test]
    fn stack_types() {
        let test = "var a = 1;
        var b = 2;
        print a + b;";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn boolean_logic() {
        let test = "print \"hi\" or 2; // \"hi\".";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
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
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn for_loop_short() {
        let test = "
        for (var b = 0; b < 10; b = b + 1) {
            print \"test\";
        }";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn for_loop_3() {
        let test = "
        { var a = \"outer a\"; }
        var temp;
        for (var b = 1; b < 10000; b = temp + b) {
            print b;
            temp = b;
        }";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn calling() {
        let test = "
        var a = \"global\";
        {
            fun showA() {
              print a;
            }
          
            showA();
            var a = \"block\";
            showA();
        }
        ";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn recursions() {
        let test = "
        fun fib(n) {
            if (n <= 1) return n;
            return fib(n - 2) + fib(n - 1);
          }
          for (var i = 0; i < 20; i = i + 1) { print fib(i); }
        ";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn if_statement() {
        let test = "
        if (true) print \"less\";
        print \"more\";
        ";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
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
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn classes() {
        let test = "
        class Breakfast {
            cook() {
              print \"Eggs a-fryin'!'\";
            }
          
            serve(who) {
              print \"Enjoy your breakfast, \" + who + \".\";
            }
        }
        ";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn classes_2() {
        let test = "
        class Bagel { eat() { print \"Crunch crunch crunch!\"; } }
        var bagel = Bagel();
        ";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn clock() {
        let test = "
        print clock();
        ";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn string_equality() {
        let test = "
        print \"x\" == \"x\";
        ";
        let mut vm = VM::new(Heap::new(0), ByteCode::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}
