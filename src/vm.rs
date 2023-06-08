use std::time;

use crate::{
    chunk::{Chunk, Op},
    common::U8_COUNT,
    compiler::compile,
    loxtr::Loxtr,
    memory::{Handle, Heap, Kind, Traceable, GC},
    object::{BoundMethod, Class, Closure, Instance, Native, Upvalue, Value},
    table::Table,
};

const MAX_FRAMES: usize = 0x40;
const STACK_SIZE: usize = MAX_FRAMES * U8_COUNT;

fn clock_native(_args: &[Value]) -> Result<Value, String> {
    match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
        Ok(duration) => Ok(Value::from(duration.as_secs_f64())),
        Err(x) => Err(x.to_string()),
    }
}

const CLOCK_NATIVE: Native = Native(clock_native);

struct CallFrame {
    ip: isize,
    slots: usize,
    closure: GC<Closure>,
}

impl CallFrame {
    fn new(slots: usize, closure: GC<Closure>) -> Self {
        Self {
            ip: -1,
            slots,
            closure,
        }
    }
    fn chunk(&self) -> &Chunk {
        &self.closure.function.chunk
    }
    fn read_byte(&mut self) -> u8 {
        self.ip += 1;
        self.chunk().read_byte(self.ip as usize)
    }

    fn jump_forward(&mut self) {
        self.ip += self.chunk().read_short(self.ip as usize + 1) as isize;
    }

    fn jump_back(&mut self) {
        self.ip -= self.chunk().read_short(self.ip as usize + 1) as isize;
    }

    fn read_constant(&mut self) -> Value {
        self.ip += 1;
        self.chunk().read_constant(self.ip as usize)
    }

    fn read_string(&mut self) -> Result<GC<Loxtr>, String> {
        let value = self.read_constant();
        Loxtr::nullable(value).ok_or_else(|| format!("'{}' is not a string", value))
    }

    fn read_upvalue(&mut self) -> GC<Upvalue> {
        let read_byte = self.read_byte() as usize;
        self.closure.upvalues[read_byte]
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
    frames: Vec<CallFrame>,
    open_upvalues: Option<GC<Upvalue>>,
    globals: Table<Value>,
    init_string: GC<Loxtr>,
    heap: Heap,
}

impl VM {
    pub fn new(mut heap: Heap) -> Self {
        let init_string = heap.intern("init");
        let mut s = Self {
            values: [Value::Nil; STACK_SIZE],
            stack_top: 0,
            frames: Vec::with_capacity(MAX_FRAMES),
            open_upvalues: None,
            globals: Table::new(),
            init_string,
            heap,
        };
        s.define_native("clock", CLOCK_NATIVE);
        s
    }
    pub fn capture_upvalue(&mut self, location: usize) -> GC<Upvalue> {
        let mut previous = None;
        let mut current = self.open_upvalues;
        while let Some(link) = current {
            if let Upvalue::Open(index, next) = *link {
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
            Some(mut obj) => {
                if let Upvalue::Open(x, _) = *obj {
                    *obj = Upvalue::Open(x, Some(created))
                }
            }
        }
        created
    }

    fn close_upvalues(&mut self, location: usize) {
        while let Some(mut link) = self.open_upvalues {
            if let Upvalue::Open(l, next) = *link {
                if l < location {
                    return;
                }
                *link = Upvalue::Closed(self.values[l]);
                self.open_upvalues = next;
            } else {
                self.open_upvalues = None;
            }
        }
    }

    fn new_obj<T: Traceable>(&mut self, t: T) -> GC<T> {
        if self.heap.needs_gc() {
            let roots = self.roots();
            self.heap.retain(roots);
        }
        self.heap.store(t)
    }

    fn roots(&mut self) -> Vec<crate::memory::Handle> {
        let mut collector = Vec::new();
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
        for frame in &self.frames {
            collector.push(Handle::from(frame.closure))
        }
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
        self.globals.trace(&mut collector);
        // no compiler roots
        #[cfg(feature = "log_gc")]
        {
            println!("collect init string");
        }
        collector.push(Handle::from(self.init_string));
        collector
    }

    fn define_native(&mut self, name: &str, native_fn: Native) {
        let key = self.heap.intern(name);
        self.push(Value::from(key));
        let value = Value::from(self.new_obj(native_fn));
        self.globals.set(key, value);
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

    fn call(&mut self, closure: GC<Closure>, arity: u8) -> Result<(), String> {
        if arity != closure.function.arity {
            return err!(
                "Expected {} arguments but got {}.",
                closure.function.arity,
                arity
            );
        }

        if self.frames.len() == MAX_FRAMES {
            return err!("Stack overflow.");
        }
        self.frames
            .push(CallFrame::new(self.stack_top - arity as usize - 1, closure));
        Ok(())
    }

    fn call_value(&mut self, callee: Value, arity: u8) -> Result<(), String> {
        if let Value::Object(handle) = callee {
            match handle.kind() {
                Kind::BoundMethod => {
                    let bm = BoundMethod::as_gc(&handle);
                    self.values[self.stack_top - arity as usize - 1] = Value::from(bm.receiver);
                    return self.call(bm.method, arity);
                }
                Kind::Class => {
                    let obj = Class::as_gc(&handle);
                    let instance = self.new_obj(Instance::new(obj));
                    self.values[self.stack_top - arity as usize - 1] = Value::from(instance);
                    if let Some(init) = obj.methods.get(self.init_string) {
                        return self.call(init, arity);
                    } else if arity > 0 {
                        return err!("Expected no arguments but got {}.", arity);
                    } else {
                        return Ok(());
                    }
                }
                Kind::Closure => {
                    return self.call(Closure::as_gc(&handle), arity);
                }
                Kind::Native => {
                    let result = Native::as_gc(&handle).0(self.tail(arity as usize)?)?;
                    self.stack_top -= arity as usize + 1;
                    self.push(result);
                    return Ok(());
                }
                _ => (),
            }
        }
        err!("Can only call functions and classes, not '{}'", callee)
    }

    fn invoke_from_class(
        &mut self,
        class: GC<Class>,
        name: GC<Loxtr>,
        arity: u8,
    ) -> Result<(), String> {
        match class.methods.get(name) {
            None => err!("Undefined property '{}'", *name),
            Some(method) => self.call(method, arity),
        }
    }

    fn invoke(&mut self, name: GC<Loxtr>, arity: u8) -> Result<(), String> {
        let value = self.peek(arity as usize);
        let instance = Instance::nullable(value).ok_or("Only instances have methods.")?;
        if let Some(property) = instance.properties.get(name) {
            self.values[self.stack_top - arity as usize - 1] = property;
            self.call_value(property, arity)
        } else {
            self.invoke_from_class(instance.class, name, arity)
        }
    }

    fn bind_method(&mut self, class: GC<Class>, name: GC<Loxtr>) -> Result<(), String> {
        match class.methods.get(name) {
            None => err!("Undefined property '{}'.", *name),
            Some(method) => {
                let instance = GC::from(self.peek(0));
                let bm = self.new_obj(BoundMethod::new(instance, method));
                self.pop();
                self.push(Value::from(bm));
                Ok(())
            }
        }
    }

    fn define_method(&mut self, name: GC<Loxtr>) -> Result<(), String> {
        if let Ok(&[a, method]) = self.tail(2) {
            let mut class = GC::<Class>::from(a);
            let before_count = class.byte_count();
            class.methods.set(name, GC::from(method));
            self.heap
                .increase_byte_count(class.byte_count() - before_count);
            self.pop();
        }
        Ok(())
    }

    fn concatenate(&mut self, a: &str, b: &str) -> Value {
        let mut c = String::new();
        c.push_str(a);
        c.push_str(b);
        Value::from(self.heap.intern(&c))
    }

    // combined to avoid gc errors
    fn push_traceable<T: Traceable>(&mut self, traceable: T) -> GC<T> {
        let obj = self.new_obj(traceable);
        self.push(Value::from(obj));
        obj
    }

    fn top_frame(&mut self) -> &mut CallFrame {
        let index = self.frames.len() - 1;
        &mut self.frames[index]
    }

    fn run(&mut self) -> Result<(), String> {
        loop {
            let instruction = Op::try_from(self.top_frame().read_byte())?;
            #[cfg(feature = "trace")]
            {
                print!("stack: ");
                for i in 0..self.stack_top {
                    print!("{};", &self.values[i]);
                }
                println!("");

                print!("globals: ");
                for (k, v) in &self.globals {
                    print!("{}:{};", **k, v)
                }
                println!("");

                let ip = self.top_frame().ip;
                println!("ip: {}", ip);
                println!("line: {}", self.top_frame().chunk().lines[ip as usize]);
                println!("op code: {:?}", instruction);
                println!();
            }
            match instruction {
                Op::Add => {
                    if let &[a, b] = self.tail(2)? {
                        if let (Some(a), Some(b)) = (Loxtr::nullable(a), Loxtr::nullable(b)) {
                            let c = self.concatenate(a.as_ref(), b.as_ref());
                            self.stack_top -= 2;
                            self.push(c);
                            continue;
                        }

                        if let (Value::Number(a), Value::Number(b)) = (a, b) {
                            self.stack_top -= 2;
                            self.push(Value::from(a + b));
                            continue;
                        }

                        return err!(
                            "Operands must be either numbers or strings, found '{}' and '{}'",
                            a,
                            b
                        );
                    }
                }
                Op::Call => {
                    let arity = self.top_frame().read_byte();
                    self.call_value(self.peek(arity as usize), arity)?;
                }
                Op::Class => {
                    let name = self.top_frame().read_string()?;
                    self.push_traceable(Class::new(name));
                }
                Op::CloseUpvalue => {
                    self.close_upvalues(self.stack_top - 1);
                    self.pop();
                }
                Op::Closure => {
                    let function = GC::from(self.top_frame().read_constant());
                    let mut closure = self.push_traceable(Closure::new(function));
                    let before_count = closure.byte_count();
                    for _ in 0..function.upvalue_count {
                        let is_local = self.top_frame().read_byte();
                        let index = self.top_frame().read_byte() as usize;
                        closure.upvalues.push(if is_local > 0 {
                            let location = self.top_frame().slots + index;
                            self.capture_upvalue(location)
                        } else {
                            self.top_frame().closure.upvalues[index]
                        })
                    }
                    self.heap
                        .increase_byte_count(closure.byte_count() - before_count)
                }
                Op::Constant => {
                    let value = self.top_frame().read_constant();
                    self.push(value)
                }
                Op::DefineGlobal => {
                    let name = self.top_frame().read_string()?;
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
                    let name = self.top_frame().read_string()?;
                    if let Some(value) = self.globals.get(name) {
                        self.push(value);
                    } else {
                        return err!("Undefined variable '{}'.", *name);
                    }
                }
                Op::GetLocal => {
                    let index = self.top_frame().slots + self.top_frame().read_byte() as usize;
                    self.push(self.values[index])
                }
                Op::GetProperty => {
                    let value = self.peek(0);
                    let instance = Instance::nullable(value)
                        .ok_or(String::from("Only instances have properties."))?;
                    let name = self.top_frame().read_string()?;
                    if let Some(value) = instance.properties.get(name) {
                        // replace instance
                        self.values[self.stack_top - 1] = value;
                    } else {
                        self.bind_method(instance.class, name)?;
                    }
                }
                Op::GetSuper => {
                    let name = self.top_frame().read_string()?;
                    let super_class = GC::from(self.pop());
                    self.bind_method(super_class, name)?;
                }
                Op::GetUpvalue => {
                    let value = match *self.top_frame().read_upvalue() {
                        Upvalue::Open(index, _) => self.values[index],
                        Upvalue::Closed(value) => value,
                    };
                    self.push(value);
                }
                Op::Greater => {
                    binary_op!(self, a, b, a > b)
                }
                Op::Inherit => {
                    if let &[a, b] = self.tail(2)? {
                        let super_class = Class::nullable(a)
                            .ok_or(String::from("Super class must be a class."))?;
                        let mut sub_class =
                            Class::nullable(b).ok_or(String::from("Sub class must be a class."))?;
                        let bytes_before = sub_class.byte_count();
                        sub_class.methods.set_all(&super_class.methods);
                        self.heap
                            .increase_byte_count(sub_class.byte_count() - bytes_before);
                        self.pop();
                    }
                }
                Op::Invoke => {
                    let name = self.top_frame().read_string()?;
                    let arity = self.top_frame().read_byte();
                    self.invoke(name, arity)?;
                }
                Op::Jump => self.top_frame().jump_forward(),
                Op::JumpIfFalse => {
                    if self.peek(0).is_falsey() {
                        self.top_frame().jump_forward();
                    } else {
                        self.top_frame().ip += 2;
                    }
                }
                Op::Less => binary_op!(self, a, b, a < b),
                Op::Loop => self.top_frame().jump_back(),
                Op::Method => {
                    let name = self.top_frame().read_string()?;
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
                Op::Print => println!("{}", self.pop()),
                Op::Return => {
                    let result = self.pop();
                    let location = self.top_frame().slots;
                    self.close_upvalues(location);
                    self.frames.pop();
                    if self.frames.is_empty() {
                        self.pop();
                        return Ok(());
                    }
                    self.stack_top = location;
                    self.push(result);
                }
                Op::SetGlobal => {
                    let name = self.top_frame().read_string()?;
                    if self.globals.set(name, self.peek(0)) {
                        self.globals.delete(name);
                        return err!("Undefined variable '{}'.", *name);
                    }
                }
                Op::SetLocal => {
                    let index = self.top_frame().read_byte() as usize;
                    self.values[self.top_frame().slots + index] = self.peek(0);
                }
                Op::SetProperty => {
                    if let &[a, b] = self.tail(2)? {
                        let mut instance = Instance::nullable(a)
                            .ok_or(String::from("Only instances have fields."))?;
                        let before_count = instance.byte_count();
                        instance.properties.set(self.top_frame().read_string()?, b);
                        self.heap
                            .increase_byte_count(instance.byte_count() - before_count);
                        self.stack_top -= 2;
                        self.push(b);
                    }
                }
                Op::SetUpvalue => {
                    let mut upvalue = self.top_frame().read_upvalue();
                    match *upvalue {
                        Upvalue::Closed(_) => *upvalue = Upvalue::Closed(self.peek(0)),
                        Upvalue::Open(index, _) => self.values[index] = self.peek(0),
                    }
                }
                Op::Subtract => binary_op!(self, a, b, a - b),
                Op::SuperInvoke => {
                    let name = self.top_frame().read_string()?;
                    let arity = self.top_frame().read_byte();
                    let super_class = GC::from(self.pop());
                    self.invoke_from_class(super_class, name, arity)?;
                }
                Op::True => self.push(Value::True),
            }
        }
    }

    fn tail(&mut self, n: usize) -> Result<&[Value], String> {
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
        let function = compile(source, &mut self.heap)?;
        self.push(Value::from(function));
        let closure = self.new_obj(Closure::new(function));
        self.pop();
        self.push(Value::from(closure));
        self.call(closure, 0)?;
        if let Err(msg) = self.run() {
            eprintln!("Error: {}", msg);
            while let Some(frame) = &self.frames.pop() {
                eprintln!(
                    "  at {} line {}",
                    *frame.closure.function,
                    frame.chunk().lines[frame.ip as usize]
                )
            }
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
        VM::new(Heap::new());
    }

    #[test]
    fn interpret_empty_string() {
        let mut vm = VM::new(Heap::new());
        assert!(vm.interpret("").is_ok())
    }

    #[test]
    fn stack_types() {
        let test = "var a = 1;
        var b = 2;
        print a + b;";
        let mut vm = VM::new(Heap::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn boolean_logic() {
        let test = "print \"hi\" or 2; // \"hi\".";
        let mut vm = VM::new(Heap::new());
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
        let mut vm = VM::new(Heap::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn for_loop_short() {
        let test = "
        for (var b = 0; b < 10; b = b + 1) {
            print \"test\";
        }";
        let mut vm = VM::new(Heap::new());
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
        let mut vm = VM::new(Heap::new());
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
        let mut vm = VM::new(Heap::new());
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
        let mut vm = VM::new(Heap::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn if_statement() {
        let test = "
        if (true) print \"less\";
        print \"more\";
        ";
        let mut vm = VM::new(Heap::new());
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
        let mut vm = VM::new(Heap::new());
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
        let mut vm = VM::new(Heap::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn classes_2() {
        let test = "
        class Bagel { eat() { print \"Crunch crunch crunch!\"; } }
        var bagel = Bagel();
        ";
        let mut vm = VM::new(Heap::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn clock() {
        let test = "
        print clock();
        ";
        let mut vm = VM::new(Heap::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn string_equality() {
        let test = "
        print \"x\" == \"x\";
        ";
        let mut vm = VM::new(Heap::new());
        let result = vm.interpret(test);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}
