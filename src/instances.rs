use std::mem;

use crate::{
    bitarray::BitArray,
    classes::ClassHandle,
    heap::{Collector, Handle, Heap, Kind, Pool},
    strings::{Map, StringHandle},
    values::Value,
};

pub type InstanceHandle = Handle<{ Kind::Instance as u8 }>;

pub struct Instances {
    classes: Vec<ClassHandle>,
    properties: Vec<Map<Value>>,
    free: Vec<InstanceHandle>,
    property_capacity: usize,
}

impl Instances {
    pub fn new() -> Self {
        Self {
            classes: Vec::new(),
            properties: Vec::new(),
            free: Vec::new(),
            property_capacity: 0,
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        if let Some(i) = self.free.pop() {
            self.classes[i.index()] = class;
            self.properties[i.index()] = Map::new();
            i
        } else {
            let i = self.classes.len() as u32;
            self.classes.push(class);
            self.properties.push(Map::new());
            InstanceHandle::from(i)
        }
    }

    pub fn to_string(&self, handle: InstanceHandle, heap: &Heap) -> String {
        format!(
            "<{} instance>",
            heap.classes
                .get_name(self.classes[handle.index()], &heap.strings)
        )
    }

    pub fn get_property(&self, handle: InstanceHandle, name: StringHandle) -> Option<Value> {
        self.properties[handle.index()].get(name)
    }

    pub fn get_class(&self, handle: InstanceHandle) -> ClassHandle {
        self.classes[handle.index()]
    }

    pub fn set_property(&mut self, a: InstanceHandle, name: StringHandle, b: Value) {
        self.properties[a.index()].set(name, b);
    }
}

impl Pool<{ Kind::Instance as u8 }> for Instances {
    fn byte_count(&self) -> usize {
        self.classes.len() * (mem::size_of::<Map<Value>>() + 4)
            + self.property_capacity * mem::size_of::<Value>()
    }
    fn trace(&self, handle: Handle<{ Kind::Instance as u8 }>, collector: &mut Collector) {
        collector.push(self.classes[handle.index()]);
        self.properties[handle.index()].trace(collector);
    }
    fn sweep(&mut self, marks: &BitArray) {
        self.free.clear();
        for i in 0..self.classes.len() {
            if !marks.get(i) {
                // no accounting for this?
                // self.classes[i] = StringHandle::EMPTY;
                self.property_capacity -= self.properties[i].capacity();
                // todo: properties.clear instead?
                self.properties[i] = Map::new();
                self.free.push(InstanceHandle::from(i as u32));
            }
        }
    }

    fn count(&self) -> usize {
        self.classes.len()
    }
}
