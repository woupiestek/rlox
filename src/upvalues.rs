use std::{mem, ops::RangeInclusive};

use crate::{
    common::STACK_SIZE,
    handles::HandleSet,
    heap::{Collector, Handle, Pool, Traceable, UPVALUE},
    values::Value,
};

pub type UpvalueHandle = Handle<UPVALUE>;

pub const HEAP_POWER: usize = 5;

/**
 * Mostly standard, but...
 *
 * Integrates a 32-ary max-heap for finding the upvalues with the highest location on the stack.
 * Shallowness is supposed to make this fast be trading the number of complex iterators
 * For simple linear searches.
 */
pub struct Upvalues {
    handles: HandleSet,
    locations: Vec<u16>,
    // special case...
    open_heap: Vec<UpvalueHandle>,
    values: Vec<Value>,
}

impl Upvalues {
    pub fn new() -> Self {
        Self {
            handles: HandleSet::new(),
            locations: Vec::new(),
            open_heap: Vec::new(),
            values: Vec::new(),
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

    fn children(index: usize) -> RangeInclusive<usize> {
        index << HEAP_POWER + 1..=(index + 1) << HEAP_POWER
    }

    fn find(&self, location: u16, index: usize) -> Option<UpvalueHandle> {
        if index >= self.open_heap.len() {
            return None;
        }
        let handle = self.open_heap[index];
        let max = self.locations[handle.index()];
        if max < location {
            return None;
        }
        if max == location {
            return Some(handle);
        }
        // the recursive case
        for j in Self::children(index) {
            if let Some(k) = self.find(location, j) {
                return Some(k);
            }
        }
        return None;
    }

    fn delete_max(&mut self) {
        let handle = if let Some(h) = self.open_heap.pop() {
            h
        } else {
            return;
        };
        let len = self.open_heap.len();
        if len == 0 {
            return;
        }
        let mut index = 0;
        let location = self.locations[handle.index()];
        loop {
            let mut child = index;
            let mut ch = handle;
            let mut max = location;
            for j in Self::children(index) {
                if j >= len {
                    break;
                }
                let handle = self.open_heap[j];
                let location = self.locations[handle.index()];
                if location > max {
                    child = j;
                    ch = handle;
                    max = location;
                }
            }
            if child == index {
                self.open_heap[index] = handle;
                return;
            }
            self.open_heap[index] = ch;
            index = child;
        }
    }

    fn add(&mut self, handle: UpvalueHandle) {
        let mut index = self.open_heap.len();
        self.open_heap.push(handle); // affects len()!
        let handle = self.open_heap[index];
        let location = self.locations[handle.index()];
        while index > 0 {
            let parent = (index - 1) >> HEAP_POWER;
            let ph = self.open_heap[parent];
            if self.locations[ph.index()] > location {
                break;
            }
            self.open_heap[index] = ph;
            index = parent;
        }
        self.open_heap[index] = handle;
    }

    pub fn open_upvalue(&mut self, location: u16) -> UpvalueHandle {
        if let Some(h) = self.find(location, 0) {
            return h;
        }
        let handle = self.store(location);
        self.add(handle);
        handle
    }

    fn store(&mut self, location: u16) -> UpvalueHandle {
        let free = self.handles.next() as usize;
        while free >= self.values.len() {
            self.values.push(Value::NIL);
            // STACK_SIZE == 0x4000 is in range
            self.locations.push(STACK_SIZE as u16);
        }
        self.locations[free] = location;
        Handle(free as u32)
    }

    // take another shot at recursion?
    pub fn close_upvalues(&mut self, location: u16, stack: &[Value]) {
        while !self.open_heap.is_empty() {
            let handle = self.open_heap[0];
            let max = self.locations[handle.index()];
            if max < location {
                return;
            }
            // close upvalue
            self.values[handle.index()] = stack[max as usize];
            self.locations[handle.index()] = STACK_SIZE as u16;
            self.delete_max();
        }
    }

    const ENTRY_SIZE: usize = mem::size_of::<Value>();

    pub fn trace_roots(&self, collector: &mut Collector) {
        for &i in &self.open_heap {
            i.trace(collector);
        }
    }

    pub fn reset_stack(&mut self) {
        self.open_heap.clear()
    }
}

impl Pool<UPVALUE> for Upvalues {
    fn byte_count(&self) -> usize {
        // that is optimistic...
        self.values.capacity() * Self::ENTRY_SIZE
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.handles.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        for &i in marked {
            self.values[i as usize].trace(collector)
        }
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {}
}
