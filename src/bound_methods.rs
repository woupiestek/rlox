use crate::{
    bitarray::BitArray,
    closures2::ClosureHandle,
    heap::{Collector, Handle, Heap, Pool, BOUND_METHOD},
    instances::InstanceHandle,
    u32s::U32s,
};

pub type BoundMethodHandle = Handle<BOUND_METHOD>;

pub struct BoundMethods {
    methods: U32s,
    receivers: Vec<InstanceHandle>,
}

impl BoundMethods {
    pub fn new() -> Self {
        Self {
            methods: U32s::new(),
            receivers: Vec::new(),
        }
    }

    pub fn bind(&mut self, instance: InstanceHandle, method: ClosureHandle) -> BoundMethodHandle {
        let i = self.methods.store(method.0);
        while self.receivers.len() < self.methods.count() {
            // pushing fake handles just in case
            self.receivers.push(Handle(0))
        }
        self.receivers[i as usize] = instance;
        BoundMethodHandle::from(i)
    }

    pub fn get_receiver(&self, handle: BoundMethodHandle) -> InstanceHandle {
        self.receivers[handle.index()]
    }

    pub fn get_method(&self, handle: BoundMethodHandle) -> ClosureHandle {
        ClosureHandle::from(self.methods.get(handle.0))
    }

    pub fn to_string(&self, handle: BoundMethodHandle, heap: &Heap) -> String {
        heap.functions
            .to_string(heap.closures.get_function(self.get_method(handle)), heap)
    }
}

impl Pool<BOUND_METHOD> for BoundMethods {
    fn byte_count(&self) -> usize {
        self.receivers.len() * 8
    }
    fn trace(&self, handle: Handle<BOUND_METHOD>, collector: &mut Collector) {
        collector.push(self.get_receiver(handle));
        collector.push(self.get_method(handle));
    }
    fn sweep(&mut self, marks: &BitArray) {
        self.methods.sweep(marks);
    }
    fn count(&self) -> usize {
        self.receivers.len()
    }
}
