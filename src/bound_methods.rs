use crate::{
    closures::ClosureHandle,
    handles::HandleSet,
    heap::{Collector, Handle, HandleColumn, Heap, Pool, BOUND_METHOD, CLOSURE, INSTANCE},
    instances::InstanceHandle,
};

pub type BoundMethodHandle = Handle<BOUND_METHOD>;

pub struct BoundMethods {
    handles: HandleSet,
    instances: HandleColumn<INSTANCE>,
    closures: HandleColumn<CLOSURE>,
}

impl BoundMethods {
    pub fn new() -> Self {
        Self {
            handles: HandleSet::new(),
            instances: HandleColumn::new(),
            closures: HandleColumn::new(),
        }
    }

    pub fn bind(&mut self, instance: InstanceHandle, method: ClosureHandle) -> BoundMethodHandle {
        let i = self.handles.next();
        self.instances.set(i, instance);
        self.closures.set(i, method);
        BoundMethodHandle::from(i)
    }

    pub fn unpack(&self, handle: BoundMethodHandle) -> (InstanceHandle, ClosureHandle) {
        (self.instances.get(handle.0), self.closures.get(handle.0))
    }

    pub fn to_string(&self, handle: BoundMethodHandle, heap: &Heap) -> String {
        heap.functions.to_string(
            heap.closures.get_function(self.closures.get(handle.0)),
            heap,
        )
    }
}

impl Pool<BOUND_METHOD> for BoundMethods {
    fn byte_count(&self) -> usize {
        self.handles.byte_count() + self.instances.byte_count() + self.closures.byte_count()
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.handles.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        self.instances.trace_all(marked, collector);
        self.closures.trace_all(marked, collector);
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {}
}
