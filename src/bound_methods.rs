use crate::{
    bitarray::BitArray,
    closures::ClosureHandle,
    heap::{Collector, Handle, Heap, Pool, BOUND_METHOD},
    instances::InstanceHandle,
};

pub type BoundMethodHandle = Handle<BOUND_METHOD>;

pub struct BoundMethods {
    receivers: Vec<InstanceHandle>,
    methods: Vec<ClosureHandle>,
    free: Vec<BoundMethodHandle>,
}

impl BoundMethods {
    pub fn new() -> Self {
        Self {
            receivers: Vec::new(),
            methods: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn bind(&mut self, instance: InstanceHandle, method: ClosureHandle) -> BoundMethodHandle {
        if let Some(i) = self.free.pop() {
            self.receivers[i.index()] = instance;
            self.methods[i.index()] = method;
            i
        } else {
            let i = self.receivers.len() as u32;
            self.receivers.push(instance);
            self.methods.push(method);
            BoundMethodHandle::from(i)
        }
    }

    pub fn get_receiver(&self, handle: BoundMethodHandle) -> InstanceHandle {
        self.receivers[handle.index()]
    }

    pub fn get_method(&self, handle: BoundMethodHandle) -> ClosureHandle {
        self.methods[handle.index()]
    }

    pub fn to_string(&self, handle: BoundMethodHandle, heap: &Heap) -> String {
        heap.functions.to_string(
            heap.closures.function_handle(self.methods[handle.index()]),
            heap,
        )
    }
}

impl Pool<BOUND_METHOD> for BoundMethods {
    fn byte_count(&self) -> usize {
        self.receivers.len() * 8
    }
    fn trace(&self, handle: Handle<BOUND_METHOD>, collector: &mut Collector) {
        collector.push(self.receivers[handle.index()]);
        collector.push(self.methods[handle.index()]);
    }
    fn sweep(&mut self, marks: &BitArray) {
        self.free.clear();
        for i in 0..self.receivers.len() {
            if !marks.has(i) {
                self.free.push(BoundMethodHandle::from(i as u32));
            }
        }
    }
    fn count(&self) -> usize {
        self.receivers.len()
    }
}
