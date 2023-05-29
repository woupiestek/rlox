use std::{collections::HashMap, time};

use crate::{
    chunk::{Chunk, Op},
    common::U8_COUNT,
    compiler::compile,
    memory::{Heap, Kind, Obj, Traceable},
    object::{BoundMethod, Class, Closure, Function, Instance, Native, Upvalue, Value},
};

const MAX_FRAMES: usize = 1 << 6;
const STACK_SIZE: usize = MAX_FRAMES * U8_COUNT;

// todo: maybe support native functions that Err
fn clock_native(_args: &[Value]) -> Value {
    match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
        Ok(duration) => Value::Number(duration.as_millis() as f64),
        Err(_) => Value::Nil, // just like how js would solve it
    }
}

const CLOCK_NATIVE: Native = Native(clock_native);

struct CallFrame {
    ip: isize,
    slots: usize,
    closure: Obj<Closure>,
}

impl CallFrame {
    fn new(slots: usize, closure: Obj<Closure>) -> Self {
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
    fn read_short(&mut self) -> u16 {
        self.ip += 2;
        self.chunk().read_short((self.ip - 1) as usize)
    }

    fn read_constant(&mut self) -> Value {
        self.ip += 1;
        self.chunk().read_constant(self.ip as usize)
    }

    fn read_string(&mut self) -> Result<Obj<String>, String> {
        String::obj_from_value(self.read_constant())
    }

    fn read_upvalue(&mut self) -> Obj<Upvalue> {
        let read_byte = self.read_byte() as usize;
        self.closure.upvalues[read_byte]
    }
    fn error<T>(&mut self, msg: &str) -> Result<T, String> {
        let last_ip = if self.ip < 1 {
            0
        } else {
            (self.ip - 1) as usize
        };
        let last_line = self.chunk().lines[last_ip];
        Err(format!("Error at line {}: {}", last_line, msg))
    }
}

macro_rules! binary_op {
    ($self:ident, $a:ident, $b:ident, $value:expr) => {{
        if let &[Value::Number($a), Value::Number($b)] = $self.tail(2)? {
            $self.count -= 2;
            $self.push($value);
        } else {
            return $self.top_frame().error("Operands must be numbers.");
        }
    }};
}

pub struct VM {
    values: [Value; STACK_SIZE],
    count: usize,
    frames: Vec<CallFrame>,
    open_upvalues: Option<Obj<Upvalue>>,
    globals: HashMap<Obj<String>, Value>,
    init_string: Obj<String>,
    heap: Heap,
    next_gc: usize,
}

impl VM {
    pub fn new(mut heap: Heap) -> Self {
        let init_string = heap.intern("init");
        let clock_string = heap.intern("clock");
        let mut s = Self {
            values: [Value::Nil; STACK_SIZE],
            count: 0,
            frames: Vec::with_capacity(MAX_FRAMES),
            open_upvalues: None,
            globals: HashMap::new(),
            init_string,
            heap,
            next_gc: 0x8000,
        };
        s.define_native(clock_string, CLOCK_NATIVE);
        s
    }
    pub fn capture_upvalue(&mut self, location: usize) -> Obj<Upvalue> {
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
            if let Upvalue::Open(l, y) = *link {
                if l < location {
                    return;
                }
                *link = Upvalue::Closed(self.values[l]);
                self.open_upvalues = y;
            } else {
                self.open_upvalues = None;
            }
        }
    }

    fn new_obj<T: Traceable>(&mut self, t: T) -> Obj<T> {
        if self.heap.count() > self.next_gc {
            let roots = self.roots();
            self.heap.collect_garbage(roots);
            self.next_gc *= 2;
        }
        self.heap.store(t)
    }

    fn roots(&mut self) -> Vec<crate::memory::Handle> {
        let mut collector = Vec::new();
        for value in self.values {
            if let Value::Object(handle) = value {
                collector.push(handle);
            }
        }
        for frame in &self.frames {
            collector.push(frame.closure.as_handle())
        }
        if let Some(upvalue) = self.open_upvalues {
            collector.push(upvalue.as_handle());
        }
        // no compiler roots
        collector.push(self.init_string.as_handle());
        collector
    }

    fn define_native(&mut self, name: Obj<String>, native_fn: Native) {
        let value = Value::Object(self.heap.store(native_fn).as_handle());
        self.globals.insert(name, value);
    }

    fn push(&mut self, value: Value) {
        self.values[self.count] = value;
        self.count += 1;
    }

    fn pop(&mut self) -> Value {
        self.count -= 1;
        self.values[self.count]
    }

    fn peek(&self, distance: usize) -> Value {
        self.values[self.count - 1 - distance]
    }

    fn call(&mut self, closure: Obj<Closure>, arg_count: u8) -> Result<(), String> {
        if arg_count != closure.function.arity {
            return self.top_frame().error(&format!(
                "Expected {} arguments but got {}.",
                closure.function.arity, arg_count
            ));
        }

        if self.frames.len() == MAX_FRAMES {
            return self.top_frame().error("Stack overflow.");
        }
        self.frames.push(CallFrame::new(self.count, closure));
        Ok(())
    }

    fn call_value(&mut self, callee: Value, arity: u8) -> Result<(), String> {
        {
            if let Value::Object(handle) = callee {
                match handle.kind() {
                    Kind::BoundMethod => {
                        let bm = BoundMethod::from_handle(&handle)?;
                        self.values[self.count - arity as usize - 1] = bm.receiver.as_value();
                        return self.call(bm.method, arity);
                    }
                    Kind::Class => {
                        let class = Class::obj_from_handle(&handle)?;
                        let instance = self.heap.store(Instance::new(class));
                        self.values[self.count - arity as usize - 1] =
                            Value::Object(instance.as_handle());
                        if let Some(&init) = class.methods.get(&self.init_string) {
                            return self.call(init, arity);
                        }
                    }
                    Kind::Closure => {
                        return self.call(Closure::obj_from_handle(&handle)?, arity);
                    }
                    Kind::Native => {
                        let native = Native::from_handle(&handle)?;
                        let result = native.0(self.tail(arity as usize)?);
                        self.count -= arity as usize + 1;
                        self.push(result);
                        return Ok(());
                    }

                    _ => (),
                }
            }
        }

        self.top_frame()
            .error("Can only call functions and classes.")
    }

    fn invoke_from_class(
        &mut self,
        class: Obj<Class>,
        name: Obj<String>,
        arity: u8,
    ) -> Result<(), String> {
        match class.methods.get(&name) {
            None => {
                return self
                    .top_frame()
                    .error(&format!("Undefined property '{}'", *name))
            }
            Some(method) => self.call(*method, arity),
        }
    }

    fn invoke(&mut self, name: Obj<String>, arity: u8) -> Result<(), String> {
        match Instance::from_value(&self.peek(arity as usize)) {
            Err(_) => self.top_frame().error("Only instances have methods."),
            Ok(instance) => {
                if let Some(property) = instance.properties.get(&name) {
                    self.values[self.count - arity as usize - 1] = *property;
                    self.call_value(*property, arity)
                } else {
                    self.invoke_from_class(instance.class, name, arity)
                }
            }
        }
    }

    fn bind_method(&mut self, class: Obj<Class>, name: Obj<String>) -> Result<(), String> {
        match class.methods.get(&name) {
            None => self
                .top_frame()
                .error(&format!("Undefined property '{}'.", *name)),
            Some(method) => {
                let instance = Instance::obj_from_value(self.peek(0))?;
                let bm = self.heap.store(BoundMethod::new(instance, *method));
                self.pop();
                self.push(bm.as_value());
                Ok(())
            }
        }
    }

    fn define_method(&mut self, name: Obj<String>) -> Result<(), String> {
        let method = self.peek(0);
        let mut class = Class::obj_from_value(method)?;
        (*class)
            .methods
            .insert(name, Closure::obj_from_value(method)?);
        self.pop();
        Ok(())
    }

    fn concatenate(&mut self, a: &str, b: &str) -> Value {
        let mut c = String::new();
        c.push_str(a);
        c.push_str(b);
        self.heap.intern(&c).as_value()
    }

    // combined to avoid gc errors
    fn push_traceable<T: Traceable>(&mut self, traceable: T) -> Obj<T> {
        let obj = self.heap.store(traceable);
        self.push(obj.as_value());
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
                for i in 0..self.count {
                    print!("{};", &self.values[i]);
                }
                println!("");

                print!("globals: ");
                for (k, v) in &self.globals {
                    print!("{}:{}, ", **k, v)
                }
                println!("");

                println!("op code: {:?}", instruction);
            }
            match instruction {
                Op::Add => {
                    if let &[a, b] = self.tail(2)? {
                        if let (Ok(a), Ok(b)) =
                            (String::obj_from_value(a), String::obj_from_value(b))
                        {
                            let c = self.concatenate(&*a, &*b);
                            self.count -= 2;
                            self.push(c);
                            continue;
                        }

                        if let (Value::Number(a), Value::Number(b)) = (a, b) {
                            self.count -= 2;
                            self.push(Value::Number(a + b));
                            continue;
                        }

                        return self.top_frame().error(
                            &(format!(
                                "Operands must be either numbers or strings, found {} and {}",
                                a.type_name(),
                                b.type_name()
                            )),
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
                    self.close_upvalues(self.count - 1);
                    self.pop();
                }
                Op::Closure => {
                    let function = Function::obj_from_value(self.top_frame().read_constant())?;
                    let mut closure = self.push_traceable(Closure::new(function));
                    for i in 0..function.upvalue_count as usize {
                        let is_local = self.top_frame().read_byte();
                        let index = self.top_frame().read_byte();
                        closure.upvalues.push(if is_local == 1 {
                            let location = self.top_frame().slots + index as usize;
                            self.capture_upvalue(location)
                        } else {
                            self.top_frame().closure.upvalues[i]
                        })
                    }
                }
                Op::Constant => {
                    let value = self.top_frame().read_constant();
                    self.push(value)
                }
                Op::DefineGlobal => {
                    let name = self.top_frame().read_string()?;
                    self.globals.insert(name, self.peek(0));
                    self.pop();
                }
                Op::Divide => binary_op!(self, a, b, Value::Number(a / b)),
                Op::Equal => {
                    let a = self.pop();
                    let b = self.pop();
                    self.push(if a == b { Value::True } else { Value::False });
                }
                Op::False => self.push(Value::False),
                Op::GetGlobal => {
                    let name = self.top_frame().read_string()?;
                    if let Some(value) = self.globals.get(&name) {
                        self.push(*value);
                    } else {
                        return self
                            .top_frame()
                            .error(&format!("Undefined variable '{}'.", *name));
                    }
                }
                Op::GetLocal => {
                    let index = self.top_frame().slots + self.top_frame().read_byte() as usize;
                    self.push(self.values[index])
                }
                Op::GetProperty => {
                    let instance = Instance::get(self.peek(0))
                        .ok_or("Only instances have properties.".to_string())?;
                    let name = self.top_frame().read_string()?;
                    if let Some(&value) = instance.properties.get(&name) {
                        // replace instance
                        self.values[self.count - 1] = value;
                    } else {
                        self.bind_method(instance.class, name)?;
                    }
                }
                Op::GetSuper => {
                    let name = self.top_frame().read_string()?;
                    let super_class = Class::obj_from_value(self.pop())?;
                    self.bind_method(super_class, name)?;
                }
                Op::GetUpvalue => {
                    let value = self.top_frame().read_upvalue().as_value();
                    self.push(value)
                }
                Op::Greater => {
                    binary_op!(self, a, b, if a > b { Value::True } else { Value::False })
                }
                Op::Inherit => {
                    if let &[a, b] = self.tail(2)? {
                        let super_class =
                            Class::get(a).ok_or("Super class must be a class.".to_string())?;
                        let mut sub_class =
                            Class::get(b).ok_or("Sub class must be a class.".to_string())?;
                        for (&k, &v) in &super_class.methods {
                            sub_class.methods.insert(k, v);
                        }
                        self.pop();
                    }
                }
                Op::Invoke => {
                    let name = self.top_frame().read_string()?;
                    let arity = self.top_frame().read_byte();
                    self.invoke(name, arity)?;
                }
                Op::Jump => self.top_frame().ip += self.top_frame().read_short() as isize,
                Op::JumpIfFalse => {
                    if self.peek(0).is_falsey() {
                        self.top_frame().ip += self.top_frame().read_short() as isize;
                    }
                }
                Op::Less => binary_op!(self, a, b, if a < b { Value::True } else { Value::False }),
                Op::Loop => self.top_frame().ip -= self.top_frame().read_short() as isize,
                Op::Method => {
                    let name = self.top_frame().read_string()?;
                    self.define_method(name)?
                }
                Op::Multiply => binary_op!(self, a, b, Value::Number(a * b)),
                Op::Negative => {
                    if let Value::Number(a) = self.peek(0) {
                        self.values[self.count - 1] = Value::Number(-a);
                    } else {
                        return self.top_frame().error("Operand must be a number.");
                    }
                }
                Op::Nil => self.push(Value::Nil),
                Op::Not => {
                    let pop = &self.pop();
                    self.push(if pop.is_falsey() {
                        Value::False
                    } else {
                        Value::True
                    })
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
                    if self.frames.len() == 0 {
                        self.pop();
                        return Ok(());
                    }
                    self.count = self.top_frame().slots;
                    self.push(result);
                }
                Op::SetGlobal => {
                    let name = self.top_frame().read_string()?;
                    if self.globals.insert(name, self.peek(0)).is_none() {
                        self.globals.remove(&name);
                        return self
                            .top_frame()
                            .error(&format!("Undefined variable '{}'.", *name));
                    }
                }
                Op::SetLocal => {
                    let index = self.top_frame().read_byte() as usize;
                    self.values[self.top_frame().slots + index] = self.peek(0);
                }
                Op::SetProperty => {
                    if let &[a, b] = self.tail(2)? {
                        let mut instance =
                            Instance::get(a).ok_or("Only instances have fields.".to_string())?;
                        instance
                            .properties
                            .insert(self.top_frame().read_string()?, b);
                        self.count -= 2;
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
                Op::Subtract => binary_op!(self, a, b, Value::Number(a - b)),
                Op::SuperInvoke => {
                    let name = self.top_frame().read_string()?;
                    let arity = self.top_frame().read_byte();
                    let super_class = Class::obj_from_value(self.pop())?;
                    self.invoke_from_class(super_class, name, arity)?;
                }
                Op::True => self.push(Value::True),
            }
        }
    }

    fn tail(&mut self, n: usize) -> Result<&[Value], String> {
        if n <= self.count {
            Ok(&self.values[self.count - n..self.count])
        } else {
            self.top_frame().error("Stack underflow")
        }
    }

    pub fn interpret(&mut self, source: &str) -> Result<(), String> {
        let function = compile(source, &mut self.heap)?;
        self.push(function.as_value());
        let closure = self.heap.store(Closure::new(function));
        self.pop();
        self.push(closure.as_value());
        self.call(closure, 0)?;
        self.run()
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
}
