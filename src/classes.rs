use std::mem;

use crate::{
    arrays::{Array, Arrays},
    closures::ClosureHandle,
    handles::{Column, HandleSet},
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
    handle_set: HandleSet,
    names: Column<SymbolHandle>,
    method_names: Vec<KeySet>,
    method_name_count: usize,
    methods: Arrays<ClosureHandle>,
    arrays: Column<Array>,
    field_names: Vec<KeySet>,
    field_name_count: usize,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            handle_set: HandleSet::new(),
            names: Column::new(),
            method_names: Vec::new(),
            method_name_count: 0,
            methods: Arrays::new(),
            arrays: Column::new(),
            field_names: Vec::new(),
            field_name_count: 0,
        }
    }

    pub fn new_class(&mut self, name: SymbolHandle) -> ClassHandle {
        let ch = Handle(self.handle_set.next());
        self.names.set(ch.0, name);
        if self.method_names.len() <= ch.index() {
            self.method_names
                .resize_with((ch.index() + 1).next_power_of_two(), KeySet::new);
        } else {
            self.method_name_count -= self.method_names[ch.index()].len();
            self.method_names[ch.index()] = KeySet::new();
        }
        if self.field_names.len() <= ch.index() {
            self.field_names
                .resize_with((ch.index() + 1).next_power_of_two(), KeySet::new);
        } else {
            self.field_name_count -= self.field_names[ch.index()].len();
            self.field_names[ch.index()] = KeySet::new();
        }
        ch
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, symbols: &'s Symbols) -> &'s str {
        symbols.get(self.names.get(ch.0))
    }

    pub fn to_string(&self, ch: ClassHandle, symbols: &Symbols) -> String {
        format!("<class {}>", self.get_name(ch, symbols))
    }

    pub fn get_method(&self, ch: ClassHandle, name: SymbolHandle) -> ClosureHandle {
        if let Some(index) = self.method_names[ch.index()].find(name) {
            self.methods.get_ref(self.arrays.get(ch.0))[index]
        } else {
            ClosureHandle::default()
        }
    }

    pub fn set_method(
        &mut self,
        ch: ClassHandle,
        name: SymbolHandle,
        method: ClosureHandle,
    ) -> bool {
        let index = self.method_names[ch.index()].add(name);
        let mut array = self.arrays.get(ch.0);
        if array.len() <= index {
            array = self.methods.realloc(array, index + 1);
            self.arrays.set(ch.0, array);
        }
        self.methods.get_mut(array)[index] = method;
        true
    }

    pub fn find_field(&self, ch: ClassHandle, key: SymbolHandle) -> Option<usize> {
        self.field_names[ch.index()].find(key)
    }

    pub fn add_field(&mut self, ch: ClassHandle, key: SymbolHandle) -> usize {
        let layout = &mut self.field_names[ch.index()];
        if let Some(index) = layout.find(key) {
            index
        } else {
            self.field_name_count += 1;
            layout.add(key)
        }
    }

    pub fn field_count(&self, ch: ClassHandle) -> usize {
        self.field_names[ch.index()].len()
    }

    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        let super_len = self.method_names[super_class.index()].len();
        if super_len == 0 {
            return;
        }
        let new_len = self.method_names[sub_class.index()].len() + super_len;
        let mut array = self.arrays.get(sub_class.0);
        if array.len() < new_len {
            array = self.methods.realloc(array, new_len);
            self.arrays.set(sub_class.0, array);
        }
        for i in 0..super_len {
            let name = self.method_names[super_class.index()].get(i);
            let method = self.methods.get_ref(self.arrays.get(super_class.0))[i];
            self.set_method(sub_class, name, method);
        }
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        // how to count the number of keys?
        self.methods.byte_count() +self.arrays.byte_count() + self.names.byte_count() + (self.field_names.capacity()+self.method_names.capacity()) * mem::size_of::<KeySet>()
        // 4 for the handle, 2 for the index, 100% memory overhead,
        + (self.field_name_count + self.method_name_count) * 12
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.handle_set.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        self.names.trace_all(marked, collector);
        for &ch in marked {
            self.field_names[ch as usize].trace(collector);
            self.method_names[ch as usize].trace(collector);
            for method in self.methods.get_ref(self.arrays.get(ch)) {
                method.trace(collector);
            }
        }
    }

    fn reset(&mut self) {
        self.handle_set.clear();
    }

    fn sweep(&mut self) {
        for i in 0..=self.handle_set.len() as u32 {
            if !self.handle_set.is_marked(i) {
                self.methods.free(self.arrays.get(i));
                self.arrays.set(i, Array::default());
            }
        }
    }
}
