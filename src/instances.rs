use std::mem;

use crate::{
    classes::{ClassHandle, Classes},
    handles::{Column, HandleSet},
    heap::{Collector, Handle, Heap, Pool, Traceable, INSTANCE},
    symbols::SymbolHandle,
    values::Value,
};

pub type InstanceHandle = Handle<INSTANCE>;

pub struct Instances {
    handles: HandleSet,
    class_handles: Column<ClassHandle>,
    // because I am tired
    values: Vec<Vec<Value>>,
    value_count: usize,
    pub classes: Classes,
}

impl Instances {
    pub fn new() -> Self {
        Self {
            handles: HandleSet::new(),
            class_handles: Column::new(),
            values: Vec::new(),
            value_count: 0,
            classes: Classes::new(),
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        let handle = self.handles.next();
        self.class_handles.set(handle, class);
        if self.values.len() <= handle as usize {
            self.values
                .resize((handle as usize + 1).next_power_of_two(), Vec::new());
        } else {
            // main concern: same object put in the place of a huge object, causing a huge memory leak.
            self.values[handle as usize].clear();
        }
        Handle(handle)
    }

    pub fn get_class<'s>(&self, ih: InstanceHandle) -> ClassHandle {
        self.class_handles.get(ih.0)
    }

    pub fn to_string(&self, ih: InstanceHandle, heap: &Heap) -> String {
        format!(
            "<{} instance>",
            heap.instances
                .classes
                .get_name(self.get_class(ih), &heap.symbols)
        )
    }

    pub fn get_property(&self, ih: InstanceHandle, key: SymbolHandle) -> Value {
        if let Some(index) = self.classes.find_field(self.class_handles.get(ih.0), key) {
            let values = &self.values[ih.0 as usize];
            if index < values.len() {
                return values[index];
            }
        }
        Value::UNDEFINED
    }

    pub fn set_property(&mut self, ih: InstanceHandle, key: SymbolHandle, value: Value) -> bool {
        let index = self.classes.add_field(self.class_handles.get(ih.0), key);
        if self.values[ih.index()].len() <= index {
            let new_len = (index + 1).next_power_of_two();
            self.value_count += new_len - self.values[ih.index()].len();
            self.values[ih.index()].resize(new_len, Value::UNDEFINED);
        }
        let new_key = self.values[ih.index()][index] == Value::UNDEFINED;
        self.values[ih.index()][index] = value;
        new_key
    }

    pub fn delete_property(&mut self, ih: InstanceHandle, key: SymbolHandle) -> bool {
        if let Some(index) = self.classes.find_field(self.class_handles.get(ih.0), key) {
            if index < self.values[ih.index()].len()
                && self.values[ih.index()][index] != Value::UNDEFINED
            {
                self.values[ih.index()][index] = Value::UNDEFINED;
                return true;
            }
        }
        false
    }
}

impl Pool<INSTANCE> for Instances {
    fn byte_count(&self) -> usize {
        self.class_handles.byte_count()
            + self.handles.byte_count()
            + self.values.capacity() * mem::size_of::<Vec<Value>>()
            //  100% memory overhead,
            + self.value_count * mem::size_of::<Value>() *2
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.handles.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        for &ih in marked {
            for &value in &self.values[ih as usize] {
                value.trace(collector);
            }
        }
        self.class_handles.trace_all(marked, collector);
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {}
}
