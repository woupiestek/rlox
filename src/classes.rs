use std::mem;

use crate::{
    bitarray::BitArray,
    closures2::ClosureHandle,
    heap::{Collector, Handle, Pool, CLASS},
    strings::{Map, StringHandle, Strings},
    u32s::U32s,
};

pub type ClassHandle = Handle<CLASS>;

pub struct Classes {
    names: U32s,
    methods: Vec<Map<ClosureHandle>>,
    method_capacity: usize,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            names: U32s::new(),
            methods: Vec::new(),
            method_capacity: 0,
        }
    }
    pub fn new_class(&mut self, name: StringHandle) -> ClassHandle {
        let i = self.names.store(name.0);
        while self.methods.len() < self.names.count() {
            self.methods.push(Map::new())
        }
        ClassHandle::from(i)
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, strings: &'s Strings) -> &'s str {
        strings.get(StringHandle(self.names.get(ch.0))).unwrap()
    }

    pub fn to_string(&self, ch: ClassHandle, strings: &Strings) -> String {
        format!("<class {}>", self.get_name(ch, strings))
    }
    pub fn get_method(&self, ch: ClassHandle, name: StringHandle) -> Option<ClosureHandle> {
        self.methods[ch.index()].get(name)
    }
    pub fn set_method(&mut self, ch: ClassHandle, name: StringHandle, method: ClosureHandle) {
        self.method_capacity -= self.methods[ch.index()].capacity();
        self.methods[ch.index()].set(name, method);
        self.method_capacity += self.methods[ch.index()].capacity();
    }

    // todo:
    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        self.methods[sub_class.index()] = self.methods[super_class.index()].clone();
        self.method_capacity += self.methods[sub_class.index()].capacity();
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        self.names.capacity() * 4
            + self.methods.len() * mem::size_of::<Map<ClosureHandle>>()
            + self.method_capacity * 4
    }
    fn trace(&self, handle: Handle<CLASS>, collector: &mut Collector) {
        collector.keys.push(StringHandle(self.names.get(handle.0)));
        self.methods[handle.index()].trace(collector);
    }

    fn sweep(&mut self, marks: &BitArray) {
        self.names.sweep(marks);
        for i in self.names.free_indices() {
            self.method_capacity -= self.methods[i as usize].capacity();
            self.methods[i as usize] = Map::new();
        }
    }
    fn count(&self) -> usize {
        self.names.count()
    }
}
