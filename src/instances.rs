use crate::{
    classes::{ClassHandle, Classes},
    handles::{Column, HandleSet},
    heap::{Collector, Handle, Heap, Pool, INSTANCE},
    symbols::SymbolHandle,
    values::Value,
};

pub type InstanceHandle = Handle<INSTANCE>;

pub struct Instances {
    handles: HandleSet,
    class_handles: Column<ClassHandle>,
    indices: Column<u32>,
    pub classes: Classes,
}

impl Instances {
    pub fn new() -> Self {
        Self {
            handles: HandleSet::new(),
            class_handles: Column::new(),
            indices: Column::new(),
            classes: Classes::new(),
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        let handle = self.handles.next();
        self.class_handles.set(handle, class);
        self.indices
            .set(handle, self.classes.properties.new_instance());
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

    fn index(&self, ih: InstanceHandle) -> usize {
        self.indices.get(ih.0) as usize
    }

    pub fn get_property(&self, ih: InstanceHandle, key: SymbolHandle) -> Value {
        self.classes.properties.get(self.index(ih), key)
    }

    pub fn set_property(&mut self, ih: InstanceHandle, key: SymbolHandle, value: Value) -> bool {
        self.classes.properties.set(self.index(ih), key, value)
    }

    pub fn delete_property(&mut self, ih: InstanceHandle, key: SymbolHandle) -> bool {
        self.classes.properties.delete(self.index(ih), key)
    }
}

impl Pool<INSTANCE> for Instances {
    fn byte_count(&self) -> usize {
        self.classes.properties.byte_count()
            + self.class_handles.byte_count()
            + self.indices.byte_count()
            + self.handles.byte_count()
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.classes.properties.mark(self.indices.get(handle))
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        let instances: Vec<u32> = marked.into_iter().map(|&i| self.indices.get(i)).collect();
        self.classes.properties.trace_all(instances, collector);
        self.class_handles.trace_all(marked, collector);
    }

    fn reset(&mut self) {
        self.classes.properties.reset();
    }

    fn sweep(&mut self) {}
}
