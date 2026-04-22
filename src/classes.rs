use std::mem;

use crate::{
    closures::ClosureHandle,
    handles::Column,
    hash_maps::HashMapPool,
    heap::{Collector, Handle, Pool, Traceable, CLASS},
    symbols::{KeySet, SymbolHandle, Symbols},
    values::Value,
};

impl Traceable for Vec<Value> {
    fn trace(&self, collector: &mut Collector) {
        for value in self {
            value.trace(collector);
        }
    }
}

pub type ClassHandle = Handle<CLASS>;

pub struct Classes {
    names: Column<SymbolHandle>,
    methods: HashMapPool<ClosureHandle, CLASS>,
    layouts: Vec<KeySet>,
    key_count: usize,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            names: Column::new(),
            methods: HashMapPool::new(),
            layouts: Vec::new(),
            key_count: 0,
        }
    }

    pub fn new_class(&mut self, name: SymbolHandle) -> ClassHandle {
        let ch = Handle(self.methods.maps.new_hash_map());
        self.names.set(ch.0, name);
        if self.layouts.len() <= ch.index() {
            self.layouts
                .resize_with((ch.index() + 1).next_power_of_two(), KeySet::new);
        } else {
            self.key_count -= self.layouts[ch.index()].len();
            self.layouts[ch.index()] = KeySet::new();
        }
        ch
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, symbols: &'s Symbols) -> &'s str {
        symbols.get(self.names.get(ch.0))
    }

    pub fn to_string(&self, ch: ClassHandle, symbols: &Symbols) -> String {
        format!("<class {}>", self.get_name(ch, symbols))
    }

    pub fn get_method(&self, ch: ClassHandle, name: SymbolHandle) -> Option<ClosureHandle> {
        self.methods.maps.get_ref(ch.0, name).copied()
    }

    pub fn set_method(
        &mut self,
        ch: ClassHandle,
        name: SymbolHandle,
        method: ClosureHandle,
    ) -> bool {
        self.methods.maps.put(ch.0, name, method)
    }

    pub fn find_field(&self, ch: ClassHandle, key: SymbolHandle) -> Option<usize> {
        self.layouts[ch.index()].find(key)
    }

    pub fn add_field(&mut self, ch: ClassHandle, key: SymbolHandle) -> usize {
        let layout = &mut self.layouts[ch.index()];
        if let Some(index) = layout.find(key) {
            index
        } else {
            self.key_count += 1;
            layout.add(key)
        }
    }

    pub fn field_count(&self, ch: ClassHandle) -> usize {
        self.layouts[ch.index()].len()
    }

    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        self.methods.maps.add_all(super_class.0, sub_class.0);
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        // how to count the number of keys?
        self.methods.byte_count() + self.names.byte_count() + self.layouts.capacity() * mem::size_of::<KeySet>()
        // 4 for the handle, 2 for the index, 100% memory overhead,
        + self.key_count * 12
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.methods.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        self.methods.trace_all(marked, collector);
        self.names.trace_all(marked, collector);
        for &ch in marked {
            self.layouts[ch as usize].trace(collector);
        }
    }

    fn reset(&mut self) {
        self.methods.reset();
    }

    fn sweep(&mut self) {
        self.methods.sweep();
    }
}
