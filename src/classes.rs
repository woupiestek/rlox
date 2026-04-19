use crate::{
    closures::ClosureHandle,
    handles::Column,
    hash_maps::HashMapPool,
    heap::{Collector, Handle, Pool, Traceable, CLASS},
    properties::Properties,
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
    names: Column<SymbolHandle>,
    methods: HashMapPool<ClosureHandle, CLASS>,
    // fixme: this was a mistake
    pub properties: Properties,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            names: Column::new(),
            methods: HashMapPool::new(),
            properties: Properties::new(),
        }
    }

    pub fn new_class(&mut self, name: SymbolHandle) -> ClassHandle {
        let ch = Handle(self.methods.maps.new_hash_map());
        self.names.set(ch.0, name);
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

    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        self.methods.maps.add_all(super_class.0, sub_class.0);
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        self.methods.byte_count() + self.names.byte_count()
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.methods.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        self.methods.trace_all(marked, collector);
        self.names.trace_all(marked, collector);
    }

    fn reset(&mut self) {
        self.methods.reset();
    }

    fn sweep(&mut self) {
        self.methods.sweep();
    }
}
