use std::mem;

use crate::{
    bitarray::BitArray,
    heap::{Collector, Handle, Pool, UPVALUE},
    values::Value,
};

pub type UpvalueHandle = Handle<UPVALUE>;

pub struct Upvalues {
    count: usize,
    open: UpvalueHeap,
    values: Vec<Value>,
}

impl Upvalues {
    pub fn new() -> Self {
        Self {
            count: 0,
            open: UpvalueHeap::new(),
            values: Vec::new(),
        }
    }

    pub fn get(&self, handle: UpvalueHandle) -> Value {
        self.values[handle.index()]
    }

    pub fn set(&mut self, handle: UpvalueHandle, value: Value) {
        self.values[handle.index()] = value
    }

    pub fn open_upvalue(&mut self, location: u16) -> UpvalueHandle {
        if let Some(h) = self.open.get(location) {
            return h;
        }
        let value = Value::from_stack_ref(location);
        let handle = self.store(value);
        self.open.add(location, handle);
        handle
    }

    fn store(&mut self, value: Value) -> Handle<4> {
        let l = self.values.len();
        if l > self.count {
            let i = UpvalueHandle::try_from(self.values.pop().unwrap()).unwrap();
            self.values[i.index()] = value;
            i
        } else {
            self.values.push(value);
            self.count += 1;
            UpvalueHandle::from(l as u32)
        }
    }

    pub fn close_upvalues(&mut self, location: u16, stack: &[Value]) {
        while let Some(p) = self.open.peek() {
            if p.0 < location {
                return;
            }
            self.set(p.1, stack[p.0 as usize]);
            self.open.delete_min();
        }
    }

    const ENTRY_SIZE: usize = mem::size_of::<Value>();

    pub fn trace_roots(&self, collector: &mut Collector) {
        for &i in &self.open.data {
            collector.push(Handle::from(i.1))
        }
    }

    pub fn reset(&mut self) {
        self.open.clear()
    }
}

impl Pool<UPVALUE> for Upvalues {
    fn byte_count(&self) -> usize {
        self.values.capacity() * Self::ENTRY_SIZE
    }
    fn trace(&self, handle: Handle<UPVALUE>, collector: &mut Collector) {
        self.values[handle.index()].trace(collector)
    }

    fn sweep(&mut self, marks: &BitArray) {
        self.values.truncate(self.count as usize);
        for i in 0..self.count {
            if !marks.has(i as usize) {
                self.values.push(Value::from(UpvalueHandle::from(i as u32)));
                self.values[i as usize] = Value::NIL;
            }
        }
    }

    fn count(&self) -> usize {
        self.values.len()
    }
}

/**
 * Binary heap
 * For each index i, the left child is 2 * i + 1, the right child is 2 * i + 2
 * Each sub tree keeps the highest locaton at the root
 *
 * Rlox needs a get operation to find open upvalues that already point to the same stack location
 * The stack locations are therefore stored twice: both as priorities for this heap, and inside the open upvalues
 * o/c this doesn't help get much for early positions of the heap, but Minificents linked list doesn't do so great
 * there either. And who knows, maybe this will just turn out to be much faster, thanks to cache considerations.
 *
 * Well, if this is not faster, at least it is more clever!
 */
pub struct UpvalueHeap {
    data: Vec<(u16, UpvalueHandle)>,
}

impl UpvalueHeap {
    fn new() -> Self {
        Self { data: Vec::new() }
    }

    fn clear(&mut self) {
        self.data.clear()
    }

    fn get(&self, location: u16) -> Option<UpvalueHandle> {
        if self.data.len() == 0 {
            return None;
        }
        let mut index = 0;
        loop {
            if index < self.data.len() {
                if self.data[index].0 == location {
                    return Some(self.data[index].1);
                }
                if self.data[index].0 > location {
                    // climb
                    index = index * 2 + 1;
                    continue;
                }
            }
            // compute the following index for a normal order traversal of the heap.
            index += 2;
            while index & 1 == 0 {
                index >>= 1;
            }
            index -= 1;

            // this means we have searched the whole heap
            if index == 0 {
                return None;
            }
        }
    }

    fn add(&mut self, location: u16, handle: UpvalueHandle) {
        // top case
        let mut index = self.data.len();
        if index == 0 {
            self.data.push((location, handle));
            return;
        }
        let mut next = (index - 1) >> 1;
        if self.data[next].0 < location {
            self.data.push((location, handle));
            return;
        }
        // drop
        self.data.push(self.data[next]);
        loop {
            index = next;
            if index == 0 {
                self.data[index] = (location, handle);
                return;
            }
            next = (index - 1) >> 1;
            if self.data[next].0 < location {
                self.data[index] = (location, handle);
                return;
            } else {
                self.data[index] = self.data[next];
            }
        }
    }

    fn delete_min(&mut self) {
        match self.data.len() {
            0 => {
                return;
            }
            1 => {
                self.data.clear();
                return;
            }
            2 => {
                self.data[0] = self.data[1];
                self.data.truncate(1);
                return;
            }
            _ => {}
        }

        let p = match self.data.pop() {
            None => {
                return;
            }
            Some(p) => p,
        };

        let mut index = 0;
        loop {
            let left = 2 * index + 1;
            let right = 2 * index + 2;
            if left >= self.data.len() {
                self.data[index] = p;
                return;
            }
            if self.data[left].0 <= p.0 {
                if right >= self.data.len() || self.data[right].0 <= p.0 {
                    self.data[index] = p;
                    return;
                }
                self.data[index] = self.data[right];
                index = right;
                continue;
            }
            // we
            if right >= self.data.len() {
                self.data[index] = self.data[left];
                self.data[left] = p;
                return;
            }
            if self.data[right].0 <= p.0 {
                self.data[index] = self.data[left];
                index = left;
                continue;
            }
            if self.data[left].0 <= self.data[right].0 {
                self.data[index] = self.data[right];
                index = right;
                continue;
            }
            self.data[index] = self.data[left];
            index = left;
        }
    }

    fn peek(&self) -> Option<(u16, UpvalueHandle)> {
        if self.data.len() == 0 {
            None
        } else {
            Some(self.data[0])
        }
    }
}
