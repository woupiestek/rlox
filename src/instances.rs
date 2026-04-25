use crate::{
    arrays::Arrays,
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
    arrays: Column<u32>,
    max_handle: u32,
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
            max_handle: 0,
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        let handle = self.handles.next();
        if self.max_handle < handle {
            self.max_handle = handle;
        }
        self.class_handles.set(handle, class);
        // todo: what if field count is 0?
        let array = self.values.alloc(self.classes.field_count(class));
        self.arrays.set(handle, array);
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
            let values = self.values.get_ref(self.arrays.get(ih.0));
            if index < values.len() {
                return values[index];
            }
        }
        Value::UNDEFINED
    }

    pub fn set_property(&mut self, ih: InstanceHandle, key: SymbolHandle, value: Value) -> bool {
        let index = self.classes.add_field(self.class_handles.get(ih.0), key);
        let mut array = self.arrays.get(ih.0);
        if self.values.len(array) <= index {
            array = self.values.realloc(array, index + 1);
            self.arrays.set(ih.0, array);
        }
        let new_key = self.values.get_ref(array)[index] == Value::UNDEFINED;
        self.values.get_mut(array)[index] = value;
        new_key
    }

    pub fn delete_property(&mut self, ih: InstanceHandle, key: SymbolHandle) -> bool {
        let array = self.arrays.get(ih.0);
        if let Some(index) = self.classes.find_field(self.class_handles.get(ih.0), key) {
            if index < self.values.len(array)
                && self.values.get_ref(array)[index] != Value::UNDEFINED
            {
                self.values.get_mut(array)[index] = Value::UNDEFINED;
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
            for &value in self.values.get_ref(self.arrays.get(ih)) {
                value.trace(collector);
            }
        }
        self.class_handles.trace_all(marked, collector);
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {
        let mut max_handle = 0;
        // self.handles.count is not what we are looking for...
        for i in 0..=self.max_handle {
            if self.handles.is_marked(i) {
                max_handle = i;
            } else {
                self.values.free(self.arrays.get(i));
            }
        }
        self.max_handle = max_handle;
    }
}
