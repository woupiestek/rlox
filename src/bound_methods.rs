use crate::{
    closures::ClosureHandle,
    handles::Handles,
    heap::{Collector, Handle, Heap, Pool, Traceable, BOUND_METHOD},
    instances::InstanceHandle,
};

pub type BoundMethodHandle = Handle<BOUND_METHOD>;

pub struct BoundMethods {
    handles: Handles,
    pairs: Vec<(InstanceHandle, ClosureHandle)>,
}

impl BoundMethods {
    pub fn new() -> Self {
        Self {
            handles: Handles::new(),
            pairs: Vec::new(),
        }
    }

    pub fn bind(&mut self, instance: InstanceHandle, method: ClosureHandle) -> BoundMethodHandle {
        let i = self.handles.next();
        while self.pairs.len() <= i as usize {
            // pushing fake handles just in case
            self.pairs.push((Handle(0), Handle(0)));
        }
        self.pairs[i as usize] = (instance, method);
        BoundMethodHandle::from(i)
    }

    pub fn unpack(&self, handle: BoundMethodHandle) -> (InstanceHandle, ClosureHandle) {
        self.pairs[handle.index()]
    }

    pub fn to_string(&self, handle: BoundMethodHandle, heap: &Heap) -> String {
        heap.functions
            .to_string(heap.closures.get_function(self.unpack(handle).1), heap)
    }
}

impl Pool<BOUND_METHOD> for BoundMethods {
    fn byte_count(&self) -> usize {
        48 + 8 * (self.pairs.capacity())
    }
    fn trace(&mut self, handle: Handle<BOUND_METHOD>, collector: &mut Collector) {
        if self.handles.mark(handle.0) {
            let (i, c) = self.unpack(handle);
            i.trace(collector);
            c.trace(collector);
        }
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {}
}
