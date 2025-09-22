use std::mem;

use crate::{
    common::STACK_SIZE,
    handles::Handles,
    heap::{Collector, Handle, Pool, UPVALUE},
    values::Value,
};

pub type UpvalueHandle = Handle<UPVALUE>;

pub struct Upvalues {
    open: UpvalueHeap,
    locations: Vec<u16>,
    values: Vec<Value>,
    handles: Handles,
}

impl Upvalues {
    pub fn new() -> Self {
        Self {
            open: UpvalueHeap::new(),
            locations: Vec::new(),
            values: Vec::new(),
            handles: Handles::new(),
        }
    }

    pub fn get(&self, handle: UpvalueHandle, stack: &[Value]) -> Value {
        let location = self.locations[handle.index()] as usize;
        if location < STACK_SIZE {
            stack[location]
        } else {
            self.values[handle.index()]
        }
    }

    pub fn set(&mut self, handle: UpvalueHandle, value: Value, stack: &mut [Value]) {
        let location = self.locations[handle.index()] as usize;
        if location < STACK_SIZE {
            stack[location] = value
        } else {
            self.values[handle.index()] = value
        }
    }

    pub fn open_upvalue(&mut self, location: u16) -> UpvalueHandle {
        if let Some(h) = self.open.get(location) {
            return Handle(h);
        }
        let handle = self.store(location);
        self.open.add(handle, location);
        Handle(handle)
    }

    fn store(&mut self, location: u16) -> u32 {
        let free = self.handles.next() as usize;
        while free >= self.values.len() {
            self.values.push(Value::NIL);
            // STACK_SIZE == 0x4000 is in range
            self.locations.push(STACK_SIZE as u16);
        }
        self.locations[free] = location;
        free as u32
    }

    pub fn close_upvalues(&mut self, location: usize, stack: &[Value]) {
        while let Some(p) = self.open.peek() {
            let lp = self.locations[p as usize] as usize;
            if lp < location {
                return;
            }
            self.open.delete_max();
            self.values[p as usize] = stack[lp];
            self.locations[p as usize] = STACK_SIZE as u16;
        }
    }

    const ENTRY_SIZE: usize = mem::size_of::<Value>();

    pub fn trace_roots(&self, collector: &mut Collector) {
        for &i in &self.open.handles {
            collector.push(Handle::<UPVALUE>::from(i))
        }
    }

    pub fn reset_stack(&mut self) {
        self.open.clear()
    }
}

impl Pool<UPVALUE> for Upvalues {
    fn byte_count(&self) -> usize {
        // that is optimistic...
        self.values.capacity() * Self::ENTRY_SIZE
    }
    fn trace(&mut self, handle: Handle<UPVALUE>, collector: &mut Collector) {
        if self.handles.mark(handle.0) {
            self.values[handle.index()].trace(collector)
        }
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {}
}

/**
 * Binary heap
 * For each index i, the left child is 2 * i + 1, the right child is 2 * i + 2
 * Each sub tree keeps the highest location at the root
 *
 * Rlox needs a get operation to find open upvalues that already point to the same stack location
 * The stack locations are therefore stored twice: both as priorities for this heap, and inside the open upvalues
 * o/c this doesn't help get much for early positions of the heap, but Munificents linked list doesn't do so great
 * there either. And who knows, maybe this will just turn out to be much faster, thanks to cache considerations.
 *
 * Well, if this is not faster, at least it is more clever!
 */
struct UpvalueHeap {
    handles: Vec<u32>,
    // added back in hopes of speeding up the structure, but results are unclear
    locations: Vec<u16>,
}

impl UpvalueHeap {
    fn new() -> Self {
        Self {
            handles: Vec::new(),
            locations: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.handles.clear();
        self.locations.clear();
    }

    fn heapify(&mut self, index: usize) -> bool {
        if index == 0 || index >= self.handles.len() {
            return false;
        }
        let parent = (index - 1) >> 1;
        if self.locations[index] <= self.locations[parent] {
            return false;
        }
        let handle = self.handles[index];
        self.handles[index] = self.handles[parent];
        self.handles[parent] = handle;
        let location = self.locations[index];
        self.locations[index] = self.locations[parent];
        self.locations[parent] = location;
        return true;
    }

    fn get(&self, location: u16) -> Option<u32> {
        if self.handles.len() == 0 {
            return None;
        }
        let mut index = 0;
        loop {
            if index < self.handles.len() {
                let li = self.locations[index];
                if li == location {
                    return Some(self.handles[index]);
                }
                if li > location {
                    // climb
                    index = index * 2 + 1;
                    continue;
                }
            }
            // compute the following index for a normal order traversal of the heap.
            index += 2;
            index >>= index.trailing_zeros();
            index -= 1;

            // this means we have searched the whole heap
            if index == 0 {
                return None;
            }
        }
    }

    fn add(&mut self, handle: u32, location: u16) {
        self.handles.push(handle);
        self.locations.push(location);
        let mut index = self.handles.len() - 1;
        while self.heapify(index) {
            index = (index - 1) >> 1;
        }
        return;
    }

    fn delete_max(&mut self) {
        match self.handles.len() {
            0 => {
                return;
            }
            1 => {
                self.handles.clear();
                return;
            }
            2 => {
                self.handles[0] = self.handles[1];
                self.handles.truncate(1);
                return;
            }
            _ => self.handles[0] = self.handles.pop().unwrap(),
        };

        let mut index = 0;
        loop {
            let left = 2 * index + 1;
            if self.heapify(left) {
                index = left;
                continue;
            }
            let right = 2 * index + 2;
            if self.heapify(right) {
                index = right;
                continue;
            }
            return;
        }
    }

    fn peek(&self) -> Option<u32> {
        if self.handles.len() == 0 {
            None
        } else {
            Some(self.handles[0])
        }
    }
}
