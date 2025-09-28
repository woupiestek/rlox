use crate::{
    classes::ClassHandle,
    hash_maps::HashMaps,
    heap::{Collector, Handle, Heap, Pool, Traceable, INSTANCE},
    strings::StringHandle,
    values::Value,
};

pub type InstanceHandle = Handle<INSTANCE>;

pub struct Instances {
    byte_count: usize,
    classes: Vec<ClassHandle>,
    properties: HashMaps<Value, INSTANCE>,
}

impl Instances {
    pub fn new() -> Self {
        Self {
            byte_count: 56,
            classes: Vec::new(),
            properties: HashMaps::new(),
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        let handle = self.properties.new_hash_map();
        while self.classes.len() <= handle.index() {
            self.classes.push(Handle(0));
        }
        self.classes[handle.index()] = class;
        handle
    }

    pub fn get_class<'s>(&self, ih: InstanceHandle) -> ClassHandle {
        self.classes[ih.index()]
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

    //
    pub fn set_property(&mut self, ih: InstanceHandle, key: StringHandle, value: Value) -> bool {
        self.properties.put(ih, key, value)
    }

    pub fn delete_property(&mut self, ih: InstanceHandle, key: StringHandle) -> bool {
        self.properties.delete(ih, key)
    }
}

impl Pool<INSTANCE> for Instances {
    fn byte_count(&self) -> usize {
        self.byte_count + self.classes.len() * 4
    }
    fn trace(&mut self, handle: Handle<INSTANCE>, collector: &mut Collector) {
        self.properties.trace(handle, collector);
        self.classes[handle.index()].trace(collector);
    }

    fn reset(&mut self) {
        self.properties.reset();
    }

    fn sweep(&mut self) {}
}
