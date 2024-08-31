use std::mem;

use crate::{
    bitarray::BitArray,
    common::UPVALUES,
    heap::{Collector, Handle},
    object::Value,
};

pub type UpvalueHandle = Handle<UPVALUES>;

pub struct Upvalues {
    // never throw away an upvalue.
    // just put its handle on the free list.
    free: Vec<u32>,
    open: OpenUpvalues,
    values: Vec<Value>,
}

impl Upvalues {
    pub fn new() -> Self {
        Self {
            free: Vec::new(),
            open: OpenUpvalues::new(),
            values: Vec::new(),
        }
    }

    pub fn get(&self, handle: UpvalueHandle) -> Value {
        self.values[handle.0 as usize]
    }

    pub fn set(&mut self, handle: UpvalueHandle, value: Value) {
        self.values[handle.0 as usize] = value
    }

    pub fn open_upvalue(&mut self, location: u16) -> UpvalueHandle {
        if let Some(h) = self.open.get(location) {
            return h;
        }
        let value = Value::StackRef(location);
        let handle = self.store(value);
        self.open.add(location, handle);
        handle
    }

    fn store(&mut self, value: Value) -> Handle<4> {
        let handle = UpvalueHandle::from(if let Some(i) = self.free.pop() {
            self.values[i as usize] = value;
            i
        } else {
            let i = self.values.len() as u32;
            self.values.push(value);
            i
        });
        handle
    }
    
    // alternative to messing with a linked list
    pub fn close(&mut self, location: u16, stack: &[Value]) {
        self.open.rotate(location);
        for i in 0..self.open.higher_locations.len() {
            self.set(
                self.open.higher_handles[i],
                stack[self.open.higher_locations[i] as usize],
            );
        }
        self.open.higher_locations.clear();
        self.open.higher_handles.clear();
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }

    const ENTRY_SIZE: usize = mem::size_of::<Value>();

    pub fn byte_count(&self) -> usize {
        self.values.capacity() * Self::ENTRY_SIZE
    }

    pub fn trace(&self, handle: UpvalueHandle, collector: &mut Collector) {
        collector.trace(self.values[handle.0 as usize])
    }

    pub fn trace_roots(&self, collector: &mut Collector) {
        for &i in &self.open.lower_handles {
            collector.upvalues.push(Handle::from(i))
        }
        for &i in &self.open.higher_handles {
            collector.upvalues.push(Handle::from(i))
        }
    }

    pub fn sweep(&mut self, marked: BitArray) {
        self.free.clear();
        for i in 0..self.values.len() {
            if !marked.get(i) {
                self.values[i] = Value::Nil;
                self.free.push(i as u32);
            }
        }
    }

    pub fn reset(&mut self) {
        self.open.clear()
    }
}


// a heap would work given that always the highest locations are dropped
// for now, a linked list mimic. Elements are moved, to keep it sorted
struct OpenUpvalues {
    pivot: u16,
    lower_locations: Vec<u16>,
    lower_handles: Vec<UpvalueHandle>,
    higher_locations: Vec<u16>,
    higher_handles: Vec<UpvalueHandle>,
}

impl OpenUpvalues {
    fn new() -> Self {
        Self {
            pivot: 0,
            lower_locations: Vec::new(),
            lower_handles: Vec::new(),
            higher_locations: Vec::new(),
            higher_handles: Vec::new(),
        }
    }

    fn rotate(&mut self, pivot: u16) {
        if pivot < self.pivot {
            // move lower to higher
            self.pivot = pivot;
            let mut i = self.lower_locations.len();
            while i > 0 {
                i -= 1;
                if self.lower_locations[i] < pivot {
                    return;
                }
                self.higher_locations
                    .push(self.lower_locations.pop().unwrap());
                self.higher_handles.push(self.lower_handles.pop().unwrap());
            }
        } else if pivot > self.pivot {
            self.pivot = pivot;
            let mut i = self.higher_handles.len();
            while i > 0 {
                i -= 1;
                if self.higher_locations[i] < pivot {
                    self.lower_locations
                        .push(self.higher_locations.pop().unwrap());
                    self.lower_handles.push(self.higher_handles.pop().unwrap());
                } else {
                    return;
                }
            }
        }
    }

    fn get(&mut self, location: u16) -> Option<UpvalueHandle> {
        self.rotate(location);
        if let Some(&l) = self.higher_locations.last() {
            if l == location {
                return Some(self.higher_handles[self.higher_handles.len() - 1]);
            }
        }
        return None;
    }

    fn add(&mut self, location: u16, handle: UpvalueHandle) {
        self.rotate(location);
        self.higher_locations.push(location);
        self.higher_handles.push(handle);
    }

    fn clear(&mut self) {
        self.higher_handles.clear();
        self.higher_locations.clear();
        self.lower_handles.clear();
        self.lower_locations.clear();
    }
}
