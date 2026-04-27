use crate::{
    arrays::{Array, Arrays},
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
    values: Arrays<Value>,
    arrays: Column<Array>,
    pub classes: Classes,
}

impl Instances {
    pub fn new() -> Self {
        Self {
            handles: HandleSet::new(),
            class_handles: Column::new(),
            values: Arrays::new(),
            arrays: Column::new(),
            classes: Classes::new(),
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        let handle = self.handles.next();
        self.class_handles.set(handle, class);
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
            let array = self.arrays.get(ih.0);
            if index < array.len() {
                return self.values[array.ptr() + index];
            }
        }
        Value::UNDEFINED
    }

    pub fn set_property(&mut self, ih: InstanceHandle, key: SymbolHandle, value: Value) -> bool {
        let index = self.classes.add_field(self.class_handles.get(ih.0), key);
        let mut array = self.arrays.get(ih.0);
        if array.len() <= index {
            array = self.values.realloc(array, index + 1);
            self.arrays.set(ih.0, array);
        }
        let new_key = self.values[array.ptr() + index] == Value::UNDEFINED;
        self.values[array.ptr() + index] = value;
        new_key
    }

    pub fn delete_property(&mut self, ih: InstanceHandle, key: SymbolHandle) -> bool {
        let array = self.arrays.get(ih.0);
        if let Some(index) = self.classes.find_field(self.class_handles.get(ih.0), key) {
            if index < array.len() && self.values[array.ptr() + index] != Value::UNDEFINED {
                self.values[array.ptr() + index] = Value::UNDEFINED;
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
            + self.values.byte_count()
            + self.arrays.byte_count()
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.handles.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        for &ih in marked {
            for &value in &self.values[self.arrays.get(ih).range()] {
                value.trace(collector);
            }
        }
        self.class_handles.trace_all(marked, collector);
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {
        for i in 0..self.handles.len() as u32 {
            if !self.handles.is_marked(i) {
                self.values.free(self.arrays.get(i));
                self.arrays.set(i, Array::default());
            }
        }
    }
}
