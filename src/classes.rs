use std::mem;

use crate::{
    closures::ClosureHandle,
    hash_maps::HashMaps,
    heap::{Collector, Handle, Pool, Traceable, CLASS},
    strings::{StringHandle, Strings},
};

pub type ClassHandle = Handle<CLASS>;

pub struct Classes {
    names: Vec<StringHandle>,
    methods: HashMaps<ClosureHandle, CLASS>,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
            methods: HashMaps::new(),
        }
    }

    pub fn new_class(&mut self, name: StringHandle) -> ClassHandle {
        let ch = self.methods.new_hash_map();
        while self.names.len() <= ch.index() {
            self.names.push(StringHandle::EMPTY);
        }
        self.names[ch.index()] = name;
        ch
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, strings: &'s Strings) -> &'s str {
        strings.get(self.names[ch.index()])
    }

    pub fn to_string(&self, ch: ClassHandle, strings: &Strings) -> String {
        format!("<class {}>", self.get_name(ch, strings))
    }

    pub fn get_method(&self, ch: ClassHandle, name: StringHandle) -> Option<ClosureHandle> {
        self.methods.get(ch, name)
    }

    pub fn set_method(
        &mut self,
        ch: ClassHandle,
        name: StringHandle,
        method: ClosureHandle,
    ) -> bool {
        self.methods.put(ch, name, method)
    }

    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        self.methods.add_all(super_class, sub_class);
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        self.methods.byte_count() + self.names.capacity() * 4 + mem::size_of::<Vec<StringHandle>>()
    }

    fn trace(&mut self, handle: Handle<CLASS>, collector: &mut Collector) {
        self.methods.trace(handle, collector);
        self.names[handle.index()].trace(collector);
    }

    fn reset(&mut self) {
        self.methods.reset();
    }

    fn sweep(&mut self) {
        self.methods.sweep();
    }
}
