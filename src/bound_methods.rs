use crate::{
    closures::ClosureHandle,
    handles::Handles,
    heap::{Collector, Handle, Heap, Pool, BOUND_METHOD},
    instances::InstanceHandle,
};

pub type BoundMethodHandle = Handle<BOUND_METHOD>;

pub struct BoundMethods {
    handles: Handles,
    methods: Vec<ClosureHandle>,
    receivers: Vec<InstanceHandle>,
}

impl BoundMethods {
    pub fn new() -> Self {
        Self {
            handles: Handles::new(),
            methods: Vec::new(),
            receivers: Vec::new(),
        }
    }

    pub fn bind(&mut self, instance: InstanceHandle, method: ClosureHandle) -> BoundMethodHandle {
        let i = self.handles.next();
        while self.receivers.len() <= i as usize {
            // pushing fake handles just in case
            self.methods.push(Handle(0));
            self.receivers.push(Handle(0));
        }
        self.methods[i as usize] = method;
        self.receivers[i as usize] = instance;
        BoundMethodHandle::from(i)
    }

    pub fn get_receiver(&self, handle: BoundMethodHandle) -> InstanceHandle {
        self.receivers[handle.index()]
    }

    pub fn get_method(&self, handle: BoundMethodHandle) -> ClosureHandle {
        self.methods[handle.index()]
    }

    pub fn to_string(&self, handle: BoundMethodHandle, heap: &Heap) -> String {
        heap.functions
            .to_string(heap.closures.get_function(self.get_method(handle)), heap)
    }
}

impl Pool<BOUND_METHOD> for BoundMethods {
    fn byte_count(&self) -> usize {
        48 + 4 * (self.receivers.capacity() + self.methods.capacity())
    }
    fn trace(&mut self, handle: Handle<BOUND_METHOD>, collector: &mut Collector) {
        if self.handles.mark(handle.0) {
            collector.push(self.get_receiver(handle));
            collector.push(self.get_method(handle));
        }
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {}
}
