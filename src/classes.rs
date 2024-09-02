use std::mem;

use crate::{
    bitarray::BitArray,
    closures::ClosureHandle,
    heap::{Collector, Handle, Pool, CLASS},
    strings::{Map, StringHandle, Strings},
};

pub type ClassHandle = Handle<CLASS>;

pub struct Classes {
    names: Vec<StringHandle>,
    methods: Vec<Map<ClosureHandle>>,
    free: Vec<ClassHandle>,
    method_capacity: usize,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
            methods: Vec::new(),
            free: Vec::new(),
            method_capacity: 0,
        }
    }
    pub fn new_class(&mut self, name: StringHandle) -> ClassHandle {
        if let Some(i) = self.free.pop() {
            self.names[i.index()] = name;
            self.methods[i.index()] = Map::new();
            i
        } else {
            let i = self.names.len() as u32;
            self.names.push(name);
            self.methods.push(Map::new());
            ClassHandle::from(i)
        }
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, strings: &'s Strings) -> &'s str {
        strings.get(self.names[ch.index()]).unwrap()
    }

    pub fn to_string(&self, ch: ClassHandle, strings: &Strings) -> String {
        format!("<class {}>", self.get_name(ch, strings))
    }
    pub fn get_method(&self, ch: ClassHandle, name: StringHandle) -> Option<ClosureHandle> {
        self.methods[ch.index()].get(name)
    }
    pub fn set_method(&mut self, ch: ClassHandle, name: StringHandle, method: ClosureHandle) {
        let before = self.methods[ch.index()].capacity();
        self.methods[ch.index()].set(name, method);
        self.method_capacity = self.methods[ch.index()].capacity() - before;
    }

    // todo:
    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        self.methods[sub_class.index()] = self.methods[super_class.index()].clone();
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        self.names.len() * (mem::size_of::<Map<ClosureHandle>>() + 4) + self.method_capacity * 4
    }
    fn trace(&self, handle: Handle<CLASS>, collector: &mut Collector) {
        collector.push(self.names[handle.index()]);
        self.methods[handle.index()].trace(collector);
    }

    fn sweep(&mut self, marks: &BitArray) {
        self.free.clear();
        self.method_capacity = 0;
        for i in 0..self.names.len() {
            if !marks.get(i) {
                self.methods[i] = Map::new();
                self.free.push(ClassHandle::from(i as u32));
            } else {
                self.method_capacity += self.methods[i].capacity();
            }
        }
    }

    fn count(&self) -> usize {
        self.names.len()
    }
}
