use std::mem;

use crate::{
    handles::Handles,
    heap::{Collector, Handle, Pool, Traceable},
    strings::StringHandle,
};

/*
 * Purpose:
 * - reuse of logic at least between globals, classes and instances
 * - never give any memory up, but reuse aggressively instead.
 */

const KNUTH_PHI: u32 = 2654435761;

struct HashMap<A: Copy + Default + Traceable> {
    count: usize,
    keys: Box<[StringHandle]>,
    values: Box<[A]>,
}

impl<A: Copy + Default + Traceable> HashMap<A> {
    fn find(&self, key: StringHandle) -> usize {
        let mask = self.keys.len() - 1;
        let mut index = (key.0.wrapping_mul(KNUTH_PHI) >> (mask as u32).leading_zeros()) as usize;
        let mut tombstone = usize::MAX;
        loop {
            match self.keys[index] {
                StringHandle::EMPTY => {
                    return if tombstone < usize::MAX {
                        tombstone
                    } else {
                        index
                    };
                }
                StringHandle::TOMBSTONE => {
                    tombstone = index;
                }
                other => {
                    if other == key {
                        return index;
                    }
                }
            }
            index = (index + 1) & mask;
        }
    }

    fn is_full(&self) -> bool {
        4 * self.count > 3 * self.keys.len()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            count: 0,
            keys: vec![StringHandle::EMPTY; capacity].into_boxed_slice(),
            values: vec![A::default(); capacity].into_boxed_slice(),
        }
    }

    pub fn get(&self, key: StringHandle) -> Option<A> {
        let index = self.find(key);
        if self.keys[index] == key {
            Some(self.values[index])
        } else {
            None
        }
    }

    // true means a new key was added
    // false means it was not
    // there is no indication of what happened to the value.
    fn put_unchecked(&mut self, key: StringHandle, value: A) -> bool {
        let index = self.find(key);
        self.values[index] = value;
        if self.keys[index] == key {
            return false;
        }
        self.keys[index] = key;
        self.count += 1;
        true
    }

    pub fn put(&mut self, key: StringHandle, value: A) -> bool {
        assert!(!self.is_full());
        self.put_unchecked(key, value)
    }

    pub fn delete(&mut self, key: StringHandle) -> bool {
        if !key.is_valid() {
            return false;
        }
        let index = self.find(key);
        if self.keys[index] == key {
            self.keys[index] = StringHandle::TOMBSTONE;
            self.values[index] = A::default();
            return true;
        }
        false
    }

    pub fn clear(&mut self) {
        self.count = 0;
        self.keys.fill(StringHandle::EMPTY);
        self.values.fill(A::default());
    }

    fn capacity(&self) -> usize {
        self.keys.len()
    }
}

impl<A: Copy + Default + Traceable> Traceable for HashMap<A> {
    fn trace(&self, collector: &mut Collector) {
        for i in 0..self.keys.len() {
            if self.keys[i].is_valid() {
                self.keys[i].trace(collector);
                self.values[i].trace(collector);
            }
        }
    }
}

pub struct HashMaps<A: Copy + Default + Traceable, const KIND: usize> {
    handles: Handles,
    active: Vec<Option<HashMap<A>>>,
    stash: Vec<Vec<HashMap<A>>>,
    hash_map_byte_count: usize,
}

impl<A: Copy + Default + Traceable, const KIND: usize> HashMaps<A, KIND> {
    pub fn new() -> Self {
        Self {
            handles: Handles::new(),
            active: Vec::new(),
            stash: Vec::new(),
            hash_map_byte_count: 0,
        }
    }

    pub fn new_hash_map(&mut self) -> Handle<KIND> {
        let next = self.handles.next();
        while self.active.len() <= next as usize {
            self.active.push(None);
        }
        Handle(next)
    }

    pub fn get(&self, handle: Handle<KIND>, key: StringHandle) -> Option<A> {
        if let Some(hash_map) = &self.active[handle.index()] {
            hash_map.get(key)
        } else {
            None
        }
    }

    fn rank(capacity: usize) -> usize {
        capacity.ilog2() as usize - 3
    }

    fn alloc(&mut self, capacity: usize) -> HashMap<A> {
        let index = Self::rank(capacity);
        if index < self.stash.len() {
            if let Some(mut hash_map) = self.stash[index].pop() {
                hash_map.clear();
                return hash_map;
            }
        }
        self.hash_map_byte_count +=
            mem::size_of::<HashMap<A>>() + (4 + mem::size_of::<A>()) * capacity;
        return HashMap::with_capacity(capacity);
    }

    fn stash(&mut self, hash_map: HashMap<A>) {
        let rank = Self::rank(hash_map.capacity());
        while self.stash.len() <= rank {
            self.stash.push(Vec::new());
        }
        self.stash[Self::rank(hash_map.capacity())].push(hash_map);
    }

    fn resize(&mut self, handle: Handle<KIND>, capacity: usize) {
        if let Some(hash_map) = &self.active[handle.index()] {
            if hash_map.capacity() >= capacity {
                return;
            }
        }
        let mut new_map = self.alloc(capacity);
        let old_map = self.active[handle.index()].take(); //.replace(new_map);
        if let Some(hash_map) = old_map {
            for i in 0..hash_map.keys.len() {
                let k = hash_map.keys[i];
                if k.is_valid() {
                    let v = hash_map.values[i];
                    new_map.put(k, v);
                }
            }
            self.stash(hash_map);
        }
        self.active[handle.index()] = Some(new_map);
    }

    fn hash_map_ref(&mut self, handle: Handle<KIND>) -> &HashMap<A> {
        self.active[handle.index()].as_ref().unwrap()
    }

    fn hash_map_mut(&mut self, handle: Handle<KIND>) -> &mut HashMap<A> {
        self.active[handle.index()].as_mut().unwrap()
    }

    pub fn put(&mut self, handle: Handle<KIND>, key: StringHandle, value: A) -> bool {
        let capacity = if let Some(hash_map) = &mut self.active[handle.index()] {
            if !hash_map.is_full() {
                return hash_map.put(key, value);
            }
            hash_map.keys.len() * 2
        } else {
            8
        };
        self.resize(handle, capacity);
        self.hash_map_mut(handle).put(key, value)
    }

    pub fn count(&self, handle: Handle<KIND>) -> usize {
        if let Some(hash_map) = &self.active[handle.index()] {
            hash_map.count
        } else {
            0
        }
    }

    pub fn add_all(&mut self, source: Handle<KIND>, target: Handle<KIND>) {
        if self.active[source.index()].is_none() {
            return;
        }

        let count = self.count(source) + self.count(target);
        let target_capacity = if count > 8 {
            ((count - 1) * 4 / 3 + 1).next_power_of_two()
        } else {
            8
        };

        self.resize(target, target_capacity);
        for i in 0..self.hash_map_ref(source).keys.len() {
            let k = self.hash_map_ref(source).keys[i];
            if k.is_valid() {
                let v = self.hash_map_ref(source).values[i];
                self.hash_map_mut(target).put(k, v);
            }
        }
    }

    pub fn delete(&mut self, handle: Handle<KIND>, key: StringHandle) -> bool {
        if let Some(hash_map) = &mut self.active[handle.index()] {
            hash_map.delete(key)
        } else {
            false
        }
    }
}

impl<A: Copy + Default + Traceable, const KIND: usize> Pool<KIND> for HashMaps<A, KIND> {
    fn byte_count(&self) -> usize {
        mem::size_of::<Self>()
            + self.active.capacity() * mem::size_of::<Option<HashMap<A>>>()
            + self.stash.capacity() * mem::size_of::<Vec<HashMap<A>>>()
            + self.hash_map_byte_count
    }

    fn trace(&mut self, handle: Handle<KIND>, collector: &mut Collector) {
        if !self.handles.mark(handle.0) {
            return;
        }
        if let Some(hash_map) = &self.active[handle.index()] {
            hash_map.trace(collector);
        }
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {
        for i in 0..self.active.len() {
            if !self.handles.is_marked(i as u32) {
                if let Some(hash_map) = self.active[i].take() {
                    self.stash(hash_map);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::values::Value;

    use super::*;

    #[test]
    pub fn put_and_get() {
        let mut properties = HashMap::<Value>::with_capacity(8);
        let key = Handle(60);
        let key2 = Handle(80);
        assert!(properties.put(key, Value::TRUE));
        assert_eq!(Some(Value::TRUE), properties.get(key));
        assert_eq!(None, properties.get(key2));
    }
}
