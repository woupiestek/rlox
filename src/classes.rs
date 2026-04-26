use crate::{
    arrays::{Array, Arrays},
    closures::ClosureHandle,
    handles::{Column, HandleSet},
    heap::{Collector, Handle, Pool, Traceable, CLASS},
    key_sets::{KeySet, KeySets},
    symbols::{SymbolHandle, Symbols},
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
    key_sets: KeySets,
    method_names: Column<KeySet>,
    methods: Arrays<ClosureHandle>,
    arrays: Column<Array>,
    field_names: Column<KeySet>,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            handle_set: HandleSet::new(),
            names: Column::new(),
            key_sets: KeySets::new(),
            method_names: Column::new(),
            methods: Arrays::new(),
            arrays: Column::new(),
            field_names: Column::new(),
        }
    }

    pub fn new_class(&mut self, name: SymbolHandle) -> ClassHandle {
        let ch = Handle(self.handle_set.next());
        self.names.set(ch.0, name);
        ch
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, symbols: &'s Symbols) -> &'s str {
        symbols.get(self.names.get(ch.0))
    }

    pub fn to_string(&self, ch: ClassHandle, symbols: &Symbols) -> String {
        format!("<class {}>", self.get_name(ch, symbols))
    }

    pub fn get_method(&self, ch: ClassHandle, name: SymbolHandle) -> ClosureHandle {
        let set = self.method_names.get(ch.0);
        if let Some(index) = self.key_sets.find(set, name) {
            self.methods.get_ref(self.arrays.get(ch.0))[index]
        } else {
            ClosureHandle::default()
        }
    }

    pub fn set_method(&mut self, ch: ClassHandle, name: SymbolHandle, method: ClosureHandle) {
        let set = self.method_names.get(ch.0);
        let mut array = self.arrays.get(ch.0);
        if let Some(index) = self.key_sets.find(set, name) {
            self.methods.get_mut(array)[index] = method;
            return;
        }
        self.method_names.set(ch.0, self.key_sets.add(set, name));
        if array.len() < set.key_len() + 1 {
            array = self.methods.realloc(array, set.key_len() + 1);
            self.arrays.set(ch.0, array);
        }
        self.methods.get_mut(array)[set.key_len()] = method;
    }

    pub fn find_field(&self, ch: ClassHandle, key: SymbolHandle) -> Option<usize> {
        let set = self.field_names.get(ch.0);
        self.key_sets.find(set, key)
    }

    pub fn add_field(&mut self, ch: ClassHandle, key: SymbolHandle) -> usize {
        if let Some(index) = self.find_field(ch, key) {
            return index;
        }
        let set = self.field_names.get(ch.0);
        let index = set.key_len();
        self.field_names.set(ch.0, self.key_sets.add(set, key));
        index
    }

    pub fn field_count(&self, ch: ClassHandle) -> usize {
        self.field_names.get(ch.0).key_len()
    }

    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        let super_set = self.method_names.get(super_class.0);
        let super_len = super_set.key_len();
        if super_len == 0 {
            return;
        }
        let new_len = self.method_names.get(sub_class.0).key_len() + super_len;
        let mut array = self.arrays.get(sub_class.0);
        if array.len() < new_len {
            array = self.methods.realloc(array, new_len);
            self.arrays.set(sub_class.0, array);
        }
        for i in 0..super_len {
            let name = self.key_sets.get(super_set, i);
            let method = self.methods.get_ref(self.arrays.get(super_class.0))[i];
            self.set_method(sub_class, name, method);
        }
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        // how to count the number of keys?
        self.methods.byte_count()
            + self.arrays.byte_count()
            + self.names.byte_count()
            + self.field_names.byte_count()
            + self.method_names.byte_count()
            + self.key_sets.byte_count()
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.handle_set.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        self.names.trace_all(marked, collector);
        for &ch in marked {
            for key in self.key_sets.get_ref(self.method_names.get(ch)) {
                key.trace(collector);
            }
            for key in self.key_sets.get_ref(self.field_names.get(ch)) {
                key.trace(collector);
            }
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
                self.key_sets.free(self.method_names.get(i));
                self.method_names.set(i, KeySet::default());
                self.key_sets.free(self.field_names.get(i));
                self.method_names.set(i, KeySet::default());
                self.methods.free(self.arrays.get(i));
                self.arrays.set(i, Array::default());
            }
        }
    }
}
