use crate::{
    closures::ClosureHandle,
    hash_maps::HashMaps,
    heap::{Collector, Handle, HandleColumn, Pool, CLASS, STRING},
    strings::{StringHandle, Strings},
};

pub type ClassHandle = Handle<CLASS>;

pub struct Classes {
    names: HandleColumn<STRING>,
    methods: HashMaps<ClosureHandle, CLASS>,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            names: HandleColumn::new(),
            methods: HashMaps::new(),
        }
    }

    pub fn new_class(&mut self, name: StringHandle) -> ClassHandle {
        let ch = self.methods.new_hash_map();
        self.names.set(ch.0, name);
        ch
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, strings: &'s Strings) -> &'s str {
        strings.get(self.names.get(ch.0))
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
