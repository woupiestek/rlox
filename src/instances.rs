use std::mem;

use crate::{
    bitarray::BitArray,
    classes::ClassHandle,
    heap::{Collector, Handle, Heap, Pool, INSTANCE},
    strings::{Map, StringHandle},
    u32s::U32s,
    values::Value,
};

pub type InstanceHandle = Handle<INSTANCE>;

pub struct Instances {
    classes: U32s,
    properties: Vec<Map<Value>>,
    property_capacity: usize,
}

impl Instances {
    pub fn new() -> Self {
        Self {
            classes: U32s::new(),
            properties: Vec::new(),
            property_capacity: 0,
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        let index = self.classes.store(class.0);
        while index >= self.properties.len() as u32 {
            self.properties.push(Map::new());
        }
        InstanceHandle::from(index)
    }

    pub fn to_string(&self, handle: InstanceHandle, heap: &Heap) -> String {
        format!(
            "<{} instance>",
            heap.classes.get_name(self.get_class(handle), &heap.strings)
        )
    }

    pub fn get_property(&self, handle: InstanceHandle, name: StringHandle) -> Option<Value> {
        self.properties[handle.index()].get(name)
    }

    pub fn get_class(&self, handle: InstanceHandle) -> ClassHandle {
        Handle::from(self.classes.get(handle.0))
    }

    pub fn set_property(&mut self, a: InstanceHandle, name: StringHandle, b: Value) {
        self.property_capacity -= self.properties[a.index()].capacity();
        self.properties[a.index()].set(name, b);
        self.property_capacity += self.properties[a.index()].capacity();
    }
}

impl Pool<INSTANCE> for Instances {
    fn byte_count(&self) -> usize {
        self.classes.capacity() * 4
            + self.properties.capacity() * mem::size_of::<Map<Value>>()
            + self.property_capacity * mem::size_of::<Value>()
    }
    fn trace(&self, handle: Handle<INSTANCE>, collector: &mut Collector) {
        collector.push(self.get_class(handle));
        self.properties[handle.index()].trace(collector);
    }
    fn sweep(&mut self, marks: &BitArray) {
        assert_eq!(self.classes.count(), self.properties.len());
        self.classes.sweep(marks);
        for i in self.classes.free_indices() {
            // always here
            self.property_capacity -= self.properties[i as usize].capacity();
            self.properties[i as usize] = Map::new();
        }
    }
    fn count(&self) -> usize {
        self.classes.count()
    }
}
