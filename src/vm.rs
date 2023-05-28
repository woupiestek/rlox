use std::{collections::HashMap, time};

use crate::{
    chunk::Op,
    common::{error, U8_COUNT},
    compiler::compile,
    memory::{Heap, Kind, Obj, Traceable},
    object::{BoundMethod, Class, Closure, Function, Instance, Native, Upvalue, Value},
    stack::Stack,
};

const MAX_FRAMES: usize = 1 << 6;
const STACK_SIZE: usize = MAX_FRAMES * U8_COUNT;

fn clock_native(_args: &[Value]) -> Value {
    match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
        Ok(duration) => Value::Number(duration.as_millis() as f64),
        Err(_) => Value::Nil, // just like how js would solve it
    }
}

const CLOCK_NATIVE: Native = Native(clock_native);

#[derive(Copy, Clone)]
struct CallFrame {
    ip: isize,
    slots: usize,
    closure: Option<Obj<Closure>>,
}

impl CallFrame {
    fn new(slots: usize) -> Self {
        Self {
            ip: -1,
            slots,
            closure: None,
        }
    }
    fn code(&self, ip: isize) -> u8 {
        self.closure.unwrap().function.chunk.code[ip as usize]
    }
    fn read_byte(&mut self) -> u8 {
        self.ip += 1;
        // this feels inefficient
        self.code(self.ip)
    }
    fn read_short(&mut self) -> u16 {
        self.ip += 2;
        ((self.code(self.ip - 1) as u16) << 8) | (self.code(self.ip) as u16)
    }

    fn read_constant(&mut self) -> Value {
        self.closure.unwrap().function.chunk.constants[self.read_byte() as usize]
    }

    fn read_string(&mut self) -> Result<Obj<String>, String> {
        String::obj_from_value(self.read_constant())
    }

    fn read_upvalue(&mut self) -> Value {
        self.closure.unwrap().upvalues[self.read_byte() as usize].as_value()
    }
}

macro_rules! binary_op {
    ($self:ident, $a:ident, $b:ident, $value:expr) => {{
        if let &[Value::Number($a), Value::Number($b)] = $self.tail(2) {
            $self.count -= 2;
            $self.push($value);
        } else {
            return error("Operands must be numbers.");
        }
    }};
}

// #define BINARY_OP(valueType, op)                    \
//   do                                                \
//   {                                                 \
//     if (!IS_NUMBER(peek(0)) || !IS_NUMBER(peek(1))) \
//     {                                               \
//       runtimeError("Operands must be numbers.");    \
//       return INTERPRET_RUNTIME_ERROR;               \
//     }                                               \
//     double b = AS_NUMBER(pop());                    \
//     double a = AS_NUMBER(pop());                    \
//     push(valueType(a op b));                        \
//   } while (false)

pub struct VM {
    values: [Value; STACK_SIZE],
    count: usize,
    frames: Stack<CallFrame>,
    open_upvalues: Option<Obj<Upvalue>>,
    globals: HashMap<Obj<String>, Value>,
    init_string: Obj<String>,
    heap: Heap,
}

impl VM {
    pub fn new() -> Self {
        let mut heap = Heap::new();
        let init_string = heap.intern("init");
        let clock_string = heap.intern("clock");
        let mut s = Self {
            values: [Value::Nil; STACK_SIZE],
            count: 0,
            frames: Stack::new(MAX_FRAMES),
            open_upvalues: None,
            globals: HashMap::new(),
            init_string,
            heap,
        };
        s.define_native(clock_string, CLOCK_NATIVE);
        s
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
        self.values[self.values.len() - 1 - distance]
    }

    fn call(&mut self, closure: Obj<Closure>, arg_count: u8) -> Result<(), String> {
        if arg_count != closure.function.arity {
            return Err(format!(
                "Expected {} arguments but got {}.",
                closure.function.arity, arg_count
            ));
        }

        if self.frames.len() == MAX_FRAMES {
            return Err("Stack overflow.".to_string());
        }
        self.frames.push(CallFrame {
            ip: 0,
            slots: 10, // self.count,
            closure: Some(closure),
        });
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
                        let result = native.0(self.tail(arity as usize));
                        self.count -= arity as usize + 1;
                        self.push(result);
                        return Ok(());
                    }

                    _ => (),
                }
            }
        }

        error("Can only call functions and classes.")
    }

    fn invoke_from_class(
        &mut self,
        class: &Class,
        name: Obj<String>,
        arity: u8,
    ) -> Result<(), String> {
        match class.methods.get(&name) {
            None => return Err(format!("Undefined property '{}'", *name)),
            Some(method) => self.call(*method, arity),
        }
    }

    fn invoke(&mut self, name: Obj<String>, arity: u8) -> Result<(), String> {
        match Instance::from_value(&self.peek(arity as usize)) {
            Err(_) => error("Only instances have methods."),
            Ok(instance) => {
                if let Some(property) = instance.properties.get(&name) {
                    self.values[self.count - arity as usize - 1] = *property;
                    self.call_value(*property, arity)
                } else {
                    self.invoke_from_class(&*instance.class, name, arity)
                }
            }
        }
    }

    fn bind_method(&mut self, class: Obj<Class>, name: Obj<String>) -> Result<(), String> {
        match class.methods.get(&name) {
            None => Err(format!("Undefined property '{}'.", *name)),
            Some(method) => {
                let instance = Instance::obj_from_value(self.peek(0))?;
                let bm = self.heap.store(BoundMethod::new(instance, *method));
                self.pop();
                self.push(bm.as_value());
                Ok(())
            }
        }
    }

    // this in difficult, because I don't fully understand upvalues
    // use location of value instead of pointer to value
    fn capture_upvalue(&mut self, location: usize) -> Obj<Upvalue> {
        let mut previous: Option<Obj<Upvalue>> = None;
        let mut current: Option<Obj<Upvalue>> = self.open_upvalues;
        while let Some(upvalue) = current {
            if upvalue.location == location {
                return upvalue;
            }
            if upvalue.location < location {
                break;
            }
            previous = current;
            current = upvalue.next;
        }
        let mut created = self.heap.store(Upvalue::new(location));
        (*created).next = current;
        match previous {
            None => {
                self.open_upvalues = Some(created);
            }
            Some(mut before) => {
                (*before).next = Some(created);
            }
        }
        return created;
    }

    fn close_upvalues(&mut self, location: usize) {
        while let Some(mut upvalue) = self.open_upvalues {
            if upvalue.location < location {
                return;
            }
            (*upvalue).closed = self.values[upvalue.location];
            (*upvalue).location = usize::MAX;
            self.open_upvalues = upvalue.next;
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

    fn run(&mut self) -> Result<(), String> {
        let mut frame = *self.frames.peek(0).unwrap();
        loop {
            match Op::try_from(frame.read_byte())? {
                Op::Add => {
                    if let (Ok(a), Ok(b)) = (
                        String::obj_from_value(self.peek(1)),
                        String::obj_from_value(self.peek(0)),
                    ) {
                        let c = self.concatenate(&*a, &*b);
                        self.count -= 2;
                        self.push(c);
                        continue;
                    }

                    if let &[Value::Number(a), Value::Number(b)] = self.tail(2) {
                        self.count -= 2;
                        self.push(Value::Number(a + b));
                        continue;
                    }

                    return error("Operands must be either numbers or strings");
                }
                Op::Call => {
                    let arity = frame.read_byte();
                    self.call_value(self.peek(arity as usize), arity)?;
                    // sometimes a new frame is pushed.
                    frame = *self.frames.peek(0).unwrap();
                }
                Op::Class => {
                    self.push_traceable(Class::new(frame.read_string()?));
                }
                Op::CloseUpvalue => {
                    self.close_upvalues(self.count - 1);
                    self.pop();
                }
                Op::Closure => {
                    let function = Function::obj_from_value(frame.read_constant())?;
                    let mut closure = self.push_traceable(Closure::new(function));
                    for i in 0..function.upvalue_count as usize {
                        let is_local = frame.read_byte();
                        let index = frame.read_byte();
                        closure.upvalues.push(if is_local == 1 {
                            self.capture_upvalue(frame.slots + index as usize)
                        } else {
                            (*frame.closure.unwrap()).upvalues[i]
                        })
                    }
                }
                Op::Constant => self.push(frame.read_constant()),
                Op::DefineGlobal => {
                    let key = frame.read_string()?;
                    self.globals.insert(key, self.peek(0));
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
                    let name = frame.read_string()?;
                    if let Some(value) = self.globals.get(&name) {
                        self.push(*value);
                    } else {
                        return Err(format!("Undefined variable '{}'.", *name));
                    }
                }
                Op::GetLocal => self.push(self.values[frame.slots + frame.read_byte() as usize]),
                Op::GetProperty => {
                    let instance = Instance::get(self.peek(0))
                        .ok_or("Only instances have properties.".to_string())?;
                    let name = frame.read_string()?;
                    if let Some(&value) = instance.properties.get(&name) {
                        // replace instance
                        self.values[self.count - 1] = value;
                    } else {
                        self.bind_method(instance.class, name)?;
                    }
                }
                Op::GetSuper => {
                    let name = frame.read_string()?;
                    let super_class = Class::obj_from_value(self.pop())?;
                    self.bind_method(super_class, name)?;
                }
                Op::GetUpvalue => self.push(frame.read_upvalue()),
                Op::Greater => {
                    binary_op!(self, a, b, if a > b { Value::True } else { Value::False })
                }
                Op::Inherit => {
                    let super_class = Class::get(self.peek(1))
                        .ok_or("Super class must be a class.".to_string())?;
                    let mut sub_class =
                        Class::get(self.peek(1)).ok_or("Sub class must be a class.".to_string())?;
                    for (&k, &v) in &super_class.methods {
                        sub_class.methods.insert(k, v);
                    }
                    self.pop();
                }
                Op::Invoke => {
                    let name = frame.read_string()?;
                    let arity = frame.read_byte();
                    self.invoke(name, arity)?;
                    frame = *self.frames.peek(0).unwrap();
                }
                Op::Jump => frame.ip += frame.read_short() as isize,
                Op::JumpIfFalse => {
                    if self.peek(0).is_falsey() {
                        frame.ip += frame.read_short() as isize;
                    }
                }
                Op::Less => binary_op!(self, a, b, if a < b { Value::True } else { Value::False }),
                Op::Loop => frame.ip -= frame.read_short() as isize,
                Op::Method => self.define_method(frame.read_string()?)?,
                Op::Multiply => binary_op!(self, a, b, Value::Number(a * b)),
                Op::Negative => {
                    if let Value::Number(a) = self.peek(0) {
                        self.values[self.count - 1] = Value::Number(-a);
                    } else {
                        return error("Operand must be a number.");
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
                Op::Print => self.pop().println(),
                _ => todo!(),
            }
        }
    }

    fn tail(&self, n: usize) -> &[Value] {
        &self.values[self.count - n..self.count]
    }

    // hiero

    pub fn interpret(&mut self, source: &str) -> Result<(), String> {
        println!("{}", source);
        compile(source, &mut self.heap)?;
        Ok(())
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
}
