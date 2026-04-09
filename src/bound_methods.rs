use crate::{
    closures::ClosureHandle,
    handles::Handles,
    heap::{Collector, Handle, Heap, Pool, Traceable, BOUND_METHOD},
    instances::InstanceHandle,
};

pub type BoundMethodHandle = Handle<BOUND_METHOD>;

pub struct BoundMethods {
    handles: Handles,
    instances: Vec<InstanceHandle>,
    closures: Vec<ClosureHandle>,
}

impl BoundMethods {
    pub fn new() -> Self {
        Self {
            handles: Handles::new(),
            instances: Vec::new(),
            closures: Vec::new(),
        }
    }

    pub fn bind(&mut self, instance: InstanceHandle, method: ClosureHandle) -> BoundMethodHandle {
        let i = self.handles.next();
        while self.instances.len() <= i as usize {
            // pushing fake handles just in case
            self.instances.push(Handle(0));
        }
        self.instances[i as usize] = instance;
        while self.closures.len() <= i as usize {
            // pushing fake handles just in case
            self.closures.push(Handle(0));
        }
        self.closures[i as usize] = method;
        BoundMethodHandle::from(i)
    }

    pub fn unpack(&self, handle: BoundMethodHandle) -> (InstanceHandle, ClosureHandle) {
        (
            self.instances[handle.index()],
            self.closures[handle.index()],
        )
    }

    pub fn to_string(&self, handle: BoundMethodHandle, heap: &Heap) -> String {
        heap.functions.to_string(
            heap.closures.get_function(self.closures[handle.index()]),
            heap,
        )
    }
}

impl Pool<BOUND_METHOD> for BoundMethods {
    fn byte_count(&self) -> usize {
        48 + 8 * self.instances.capacity()
    }
    fn trace(&mut self, collector: &mut Collector) {
        let marked: Vec<u32> = self.handles.mark_all(&mut collector.handles[BOUND_METHOD]);
        for &h in &marked {
            self.instances[h as usize].trace(collector)
        }
        for &h in &marked {
            self.closures[h as usize].trace(collector)
        }
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {}
}
