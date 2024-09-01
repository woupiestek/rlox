use std::time;

use crate::{
    call_stack::CallStack,
    classes::ClassHandle,
    closures::ClosureHandle,
    common::U8_COUNT,
    compiler::compile,
    functions::FunctionHandle,
    heap::{Collector, Heap},
    natives::Natives,
    op::Op,
    strings::{Map, StringHandle},
    upvalues::UpvalueHandle,
    values::Value,
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
        let tail = $self.tail(2)?;
        if let &[Value::Number($a), Value::Number($b)] = tail {
            $self.stack_top -= 2;
            $self.push(Value::from($value));
        } else {
            return err!("Operands must be numbers, but were {:?}.", tail);
        }
    }};
}

pub struct VM {
    values: [Value; STACK_SIZE],
    stack_top: usize,
    call_stack: CallStack<MAX_FRAMES>,
    globals: Map<Value>,
    init_string: StringHandle,
    heap: Heap,
    natives: Natives,
}

impl VM {
    pub fn new() -> Self {
        let mut heap = Heap::new();
        let init_string = heap.strings.put("init");
        let mut s = Self {
            values: [Value::Nil; STACK_SIZE],
            stack_top: 0,
            call_stack: CallStack::new(),
            globals: Map::new(),
            init_string,
            heap,
            natives: Natives::new(),
        };
        s.define_native("clock", clock_native);
        s
    }

    pub fn capture_upvalue(&mut self, location: usize) -> UpvalueHandle {
        self.collect_garbage_if_needed();
        self.heap.upvalues.open_upvalue(location as u16)
    }

    fn close_upvalues(&mut self, location: usize) {
        self.heap.upvalues.close(location as u16, &self.values);
    }

    fn collect_garbage_if_needed(&mut self) {
        if self.heap.needs_gc() {
            let collector = self.roots();
            self.heap.retain(collector);
        }
    }

    fn roots(&mut self) -> Collector {
        let mut collector = Collector::new(&self.heap);
        #[cfg(feature = "log_gc")]
        {
            println!("collect stack objects");
        }
        for i in 0..self.stack_top {
            self.values[i].trace(&mut collector);
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
        self.heap.upvalues.trace_roots(&mut collector);
        #[cfg(feature = "log_gc")]
        {
            println!("collect globals");
        }
        self.globals.trace(&mut collector);
        // no compiler roots
        #[cfg(feature = "log_gc")]
        {
            println!("collect init string");
        }
        self.heap.functions.trace_roots(&mut collector);
        collector.push(self.init_string);
        collector
    }

    fn define_native(
        &mut self,
        name: &str,
        native_fn: fn(args: &[Value]) -> Result<Value, String>,
    ) {
        let key = self.heap.strings.put(name);
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

    fn call(&mut self, closure: ClosureHandle, arity: u8) -> Result<(), String> {
        let handle = self.heap.closures.function_handle(closure);
        let expected = self.heap.functions.arity(handle);
        if arity != expected {
            return err!("Expected {} arguments but got {}.", expected, arity);
        }
        self.call_stack
            .push(self.stack_top - arity as usize - 1, closure)
    }

    fn call_value(&mut self, callee: Value, arity: u8) -> Result<(), String> {
        match callee {
            Value::Class(handle) => {
                self.collect_garbage_if_needed();
                let instance = self.heap.instances.new_instance(handle);
                // self.push(Value::Instance(instance));
                self.values[self.stack_top - arity as usize - 1] = Value::Instance(instance);
                if let Some(init) = self.heap.classes.get_method(handle, self.init_string) {
                    return self.call(init, arity);
                } else if arity > 0 {
                    return err!("Expected no arguments but got {}.", arity);
                } else {
                    return Ok(());
                }
            }
            Value::BoundMethod(handle) => {
                let receiver = self.heap.bound_methods.get_receiver(handle);
                self.values[self.stack_top - arity as usize - 1] = Value::Instance(receiver);
                let method = self.heap.bound_methods.get_method(handle);
                return self.call(method, arity);
            }
            Value::Native(handle) => {
                let result = self.natives.call(handle, self.tail(arity as usize)?)?;
                self.stack_top -= arity as usize + 1;
                self.push(result);
                return Ok(());
            }
            Value::Closure(ch) => return self.call(ch, arity),
            _ => err!(
                "Can only call functions and classes, not '{}'",
                callee.to_string(&self.heap)
            ),
        }
    }

    fn invoke_from_class(
        &mut self,
        class: ClassHandle,
        name: StringHandle,
        arity: u8,
    ) -> Result<(), String> {
        match self.heap.classes.get_method(class, name) {
            None => err!(
                "Undefined property '{}'",
                self.heap.strings.get(name).unwrap()
            ),
            Some(method) => self.call(method, arity),
        }
    }

    fn invoke(&mut self, name: StringHandle, arity: u8) -> Result<(), String> {
        let handle = self.peek(arity as usize).as_instance()?;
        if let Some(property) = self.heap.instances.get_property(handle, name) {
            self.values[self.stack_top - arity as usize - 1] = property;
            self.call_value(property, arity)
        } else {
            self.invoke_from_class(self.heap.instances.get_class(handle), name, arity)
        }
    }

    fn bind_method(&mut self, class: ClassHandle, name: StringHandle) -> Result<(), String> {
        match self.heap.classes.get_method(class, name) {
            None => err!(
                "Undefined property '{}'.",
                self.heap.strings.get(name).unwrap()
            ),
            Some(method) => {
                if let Value::Instance(instance) = self.peek(0) {
                    self.collect_garbage_if_needed();
                    let bm = self.heap.bound_methods.bind(instance, method);
                    self.pop();
                    self.push(Value::BoundMethod(bm));
                    Ok(())
                } else {
                    err!(
                        "Cannot bind method {} to {}",
                        self.heap
                            .functions
                            .to_string(self.heap.closures.function_handle(method), &self.heap),
                        self.heap.strings.get(name).unwrap()
                    )
                }
            }
        }
    }

    fn define_method(&mut self, name: StringHandle) -> Result<(), String> {
        if let Ok(&[Value::Class(class), Value::Closure(method)]) = self.tail(2) {
            self.heap.classes.set_method(class, name, method);
            self.pop();
            Ok(())
        } else {
            err!("Method definition failed")
        }
    }

    fn run(&mut self) -> Result<(), String> {
        loop {
            let instruction = Op::from(self.call_stack.read_byte(&self.heap));
            #[cfg(feature = "trace")]
            {
                print!("stack: ");
                for i in 0..self.stack_top {
                    print!(
                        "{};",
                        &self.values[i].to_string(&self.heap, &self.heap.functions)
                    );
                }
                println!("");

                print!("globals: ");
                for k in self.globals.keys() {
                    print!(
                        "{}:{};",
                        self.heap.get_str(k),
                        self.globals
                            .get(k)
                            .unwrap()
                            .to_string(&self.heap, &self.heap.functions)
                    )
                }
                println!("");

                self.call_stack.print_trace(&self.heap);
                println!("op code: {:?}", instruction);
                println!();
            }
            match instruction {
                Op::Add => {
                    if let &[a, b] = self.tail(2)? {
                        if let (Value::String(a), Value::String(b)) = (a, b) {
                            let c = self.heap.strings.concat(a, b).ok_or("Missing strings")?;
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
                            a.to_string(&self.heap),
                            b.to_string(&self.heap),
                        );
                    }
                }
                Op::Call => {
                    let arity = self.call_stack.read_byte(&self.heap);
                    self.call_value(self.peek(arity as usize), arity)?;
                }
                Op::Class => {
                    let name = self.call_stack.read_string(&self.heap)?;
                    self.collect_garbage_if_needed();
                    let new_class = self.heap.classes.new_class(name);
                    self.push(Value::Class(new_class));
                }
                Op::CloseUpvalue => {
                    self.close_upvalues(self.stack_top - 1);
                    self.pop();
                }
                Op::Closure => {
                    let function = self.call_stack.read_constant(&self.heap).as_function()?;
                    // garbage collection risks?
                    self.collect_garbage_if_needed();
                    let closure = self
                        .heap
                        .closures
                        .new_closure(function, &self.heap.functions);
                    self.push(Value::Closure(closure));
                    let capacity = self.heap.functions.upvalue_count(function);
                    for i in 0..capacity {
                        let is_local = self.call_stack.read_byte(&self.heap);
                        let index = self.call_stack.read_byte(&self.heap) as usize;
                        let uh = if is_local > 0 {
                            let location = self.call_stack.slot() + index;
                            self.capture_upvalue(location)
                        } else {
                            self.call_stack.upvalue(index, &self.heap)?
                        };
                        self.heap.closures.set_upvalue(closure, i, uh);
                    }
                }
                Op::Constant => {
                    let value = self.call_stack.read_constant(&self.heap);
                    self.push(value)
                }
                Op::DefineGlobal => {
                    let name = self.call_stack.read_string(&self.heap)?;
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
                    let name = self.call_stack.read_string(&self.heap)?;
                    if let Some(value) = self.globals.get(name) {
                        self.push(value);
                    } else {
                        return err!(
                            "Undefined variable '{}'.",
                            self.heap.strings.get(name).unwrap()
                        );
                    }
                }
                Op::GetLocal => {
                    let index =
                        self.call_stack.slot() + self.call_stack.read_byte(&self.heap) as usize;
                    self.push(self.values[index])
                }
                Op::GetProperty => {
                    let handle = self.peek(0).as_instance()?;
                    let name = self.call_stack.read_string(&self.heap)?;
                    if let Some(value) = self.heap.instances.get_property(handle, name) {
                        // replace instance
                        self.values[self.stack_top - 1] = value;
                    } else {
                        self.bind_method(self.heap.instances.get_class(handle), name)?;
                    }
                }
                Op::GetSuper => {
                    let name = self.call_stack.read_string(&self.heap)?;
                    let super_class = self.pop().as_class()?;
                    self.bind_method(super_class, name)?;
                }
                Op::GetUpvalue => {
                    let value = self
                        .heap
                        .upvalues
                        .get(self.call_stack.read_upvalue(&self.heap)?);
                    if let Value::StackRef(location) = value {
                        self.push(self.values[location as usize]);
                    } else {
                        self.push(value);
                    }
                }
                Op::Greater => {
                    binary_op!(self, a, b, a > b)
                }
                Op::Inherit => {
                    if let &[Value::Class(super_class), Value::Class(sub_class)] = self.tail(2)? {
                        self.heap.classes.clone_methods(super_class, sub_class);
                        self.pop();
                    } else {
                        return err!("Super and sub classes must be classes");
                    }
                }
                Op::Invoke => {
                    let name = self.call_stack.read_string(&self.heap)?;
                    let arity = self.call_stack.read_byte(&self.heap);
                    self.invoke(name, arity)?;
                }
                Op::Jump => self.call_stack.jump_forward(&self.heap),
                Op::JumpIfFalse => {
                    if self.peek(0).is_falsey() {
                        self.call_stack.jump_forward(&self.heap);
                    } else {
                        self.call_stack.skip();
                    }
                }
                Op::Less => binary_op!(self, a, b, a < b),
                Op::Loop => self.call_stack.jump_back(&self.heap),
                Op::Method => {
                    let name = self.call_stack.read_string(&self.heap)?;
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
                Op::Print => println!("{}", self.pop().to_string(&self.heap)),
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
                    let name = self.call_stack.read_string(&self.heap)?;
                    if !self.globals.set(name, self.peek(0)) {
                        self.globals.delete(name);
                        return err!(
                            "Undefined variable '{}'.",
                            self.heap.strings.get(name).unwrap()
                        );
                    }
                }
                Op::SetLocal => {
                    let index = self.call_stack.read_byte(&self.heap) as usize;
                    self.values[self.call_stack.slot() + index] = self.peek(0);
                }
                Op::SetProperty => {
                    if let &[Value::Instance(a), b] = self.tail(2)? {
                        self.heap.instances.set_property(
                            a,
                            self.call_stack.read_string(&self.heap)?,
                            b,
                        );
                        self.stack_top -= 2;
                        self.push(b);
                    }
                }
                Op::SetUpvalue => {
                    let upvalue = self.call_stack.read_upvalue(&self.heap)?;
                    let value = self.heap.upvalues.get(upvalue);
                    if let Value::StackRef(location) = value {
                        self.values[location as usize] = self.peek(0)
                    } else {
                        self.heap.upvalues.set(upvalue, self.peek(0))
                    }
                }
                Op::Subtract => binary_op!(self, a, b, a - b),
                Op::SuperInvoke => {
                    let name = self.call_stack.read_string(&self.heap)?;
                    let arity = self.call_stack.read_byte(&self.heap);
                    let super_class = self.pop().as_class()?;
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
        self.heap.upvalues.reset();
    }

    pub fn interpret(&mut self, source: &str) -> Result<(), String> {
        compile(source, &mut self.heap)?;
        #[cfg(feature = "trace")]
        {
            use crate::debug::Disassembler;
            Disassembler::disassemble(&self.heap);
        }
        let closure = self
            .heap
            .closures
            .new_closure(FunctionHandle::MAIN, &self.heap.functions);
        self.push(Value::Closure(closure));
        self.call(closure, 0)?;
        if let Err(msg) = self.run() {
            eprintln!("Error: {}", msg);
            self.call_stack.print_stack_trace(&self.heap);
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
        VM::new();
    }

    #[test]
    fn interpret_empty_string() {
        let mut vm = VM::new();
        assert!(vm.interpret("").is_ok())
    }

    #[test]
    fn stack_types() {
        let test = "var a = 1;
        var b = 2;
        print a + b;";
        let mut vm = VM::new();
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn boolean_logic() {
        let test = "print \"hi\" or 2; // \"hi\".";
        let mut vm = VM::new();
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
        let mut vm = VM::new();
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn for_loop_short() {
        let test = "
        for (var b = 0; b < 10; b = b + 1) {
            print \"test\";
        }";
        let mut vm = VM::new();
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
        let mut vm = VM::new();
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
        let mut vm = VM::new();
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
        let mut vm = VM::new();
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn if_statement() {
        let test = "
        if (true) print \"less\";
        print \"more\";
        ";
        let mut vm = VM::new();
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
        let mut vm = VM::new();
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
        let mut vm = VM::new();
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn classes_2() {
        let test = "
        class Bagel { eat() { print \"Crunch crunch crunch!\"; } }
        var bagel = Bagel();
        ";
        let mut vm = VM::new();
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn clock() {
        let test = "
        print clock();
        ";
        let mut vm = VM::new();
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn string_equality() {
        let test = "
        print \"x\" == \"x\";
        ";
        let mut vm = VM::new();
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}
