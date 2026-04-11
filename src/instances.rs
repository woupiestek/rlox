use crate::{
    classes::ClassHandle,
    hash_maps::HashMaps,
    heap::{Collector, Handle, HandleColumn, Heap, Pool, CLASS, INSTANCE},
    strings::StringHandle,
    values::Value,
};

pub type InstanceHandle = Handle<INSTANCE>;

pub struct Instances {
    // wrapper for Vec<u32> with shared functionality?
    classes: HandleColumn<CLASS>,
    properties: HashMaps<Value, INSTANCE>,
}

impl Instances {
    pub fn new() -> Self {
        Self {
            classes: HandleColumn::new(),
            properties: HashMaps::new(),
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        let handle = self.properties.new_hash_map();
        self.classes.set(handle.0, class);
        handle
    }

    pub fn get_class<'s>(&self, ih: InstanceHandle) -> ClassHandle {
        self.classes.get(ih.0)
    }

    pub fn to_string(&self, ih: InstanceHandle, heap: &Heap) -> String {
        format!(
            "<{} instance>",
            heap.classes.get_name(self.get_class(ih), &heap.strings)
        )
    }

    pub fn get_property(&self, ih: InstanceHandle, key: StringHandle) -> Option<Value> {
        self.properties.get(ih, key)
    }

    pub fn set_property(&mut self, ih: InstanceHandle, key: StringHandle, value: Value) -> bool {
        self.properties.put(ih, key, value)
    }

    pub fn delete_property(&mut self, ih: InstanceHandle, key: StringHandle) -> bool {
        self.properties.delete(ih, key)
    }
}

impl Pool<INSTANCE> for Instances {
    fn byte_count(&self) -> usize {
        self.properties.byte_count() + self.classes.byte_count()
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.properties.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        self.properties.trace_all(marked, collector);
        self.classes.trace_all(marked, collector);
    }

    fn reset(&mut self) {
        self.properties.reset();
    }

    fn sweep(&mut self) {
        self.properties.sweep();
    }
}
