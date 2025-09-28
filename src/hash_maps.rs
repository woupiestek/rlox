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

    pub fn reset(&mut self) {
        self.count = 0;
        self.keys.fill(StringHandle::EMPTY);
        self.values.fill(A::default());
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

// keep unused maps
struct SizePool<A: Copy + Default + Traceable> {
    capacity: usize,
    handles: Handles,
    hash_maps: Vec<HashMap<A>>,
}

impl<A: Copy + Default + Traceable> SizePool<A> {
    fn for_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            handles: Handles::new(),
            hash_maps: Vec::new(),
        }
    }

    fn alloc(&mut self) -> u32 {
        let next = self.handles.next();
        while self.hash_maps.len() <= next as usize {
            self.hash_maps.push(HashMap::with_capacity(self.capacity));
        }
        next
    }

    fn free(&mut self, handle: u32) {
        if self.handles.unmark(handle) {
            self.hash_maps[handle as usize].reset();
        }
    }

    // another reason to not let HahsMaps resize themselves:
    // traversing all object to finds their sizes is a lot of work...
    fn byte_count(&self) -> usize {
        mem::size_of::<Self>()
            + mem::size_of::<HashMap<A>>() * self.hash_maps.capacity()
            + (4 + mem::size_of::<A>()) * self.capacity * self.hash_maps.len()
    }
}

pub struct HashMaps<A: Copy + Default + Traceable, const KIND: usize> {
    handles: Handles,
    pools: Vec<SizePool<A>>,
    rank: Vec<u8>,
    index: Vec<u32>,
}

impl<A: Copy + Default + Traceable, const KIND: usize> HashMaps<A, KIND> {
    pub fn new() -> Self {
        Self {
            pools: Vec::new(),
            handles: Handles::new(),
            rank: Vec::new(),
            index: Vec::new(), // rank = 0, no index needed...
        }
    }

    fn pool_ref(&self, rank: u8) -> &SizePool<A> {
        &self.pools[rank as usize - 3]
    }

    fn pool_mut(&mut self, rank: u8) -> &mut SizePool<A> {
        while self.pools.len() <= rank as usize - 3 {
            let capacity = 1 << (3 + self.pools.len());
            self.pools.push(SizePool::for_capacity(capacity))
        }
        &mut self.pools[rank as usize - 3]
    }
    fn hash_map_ref(&self, rank: u8, index: u32) -> &HashMap<A> {
        &self.pool_ref(rank).hash_maps[index as usize]
    }

    fn hash_map_mut(&mut self, rank: u8, index: u32) -> &mut HashMap<A> {
        &mut self.pool_mut(rank).hash_maps[index as usize]
    }

    pub fn new_hash_map(&mut self) -> Handle<KIND> {
        let next = self.handles.next();

        while self.rank.len() <= next as usize {
            self.rank.push(0);
            self.index.push(0);
        }

        Handle(next)
    }

    pub fn get(&self, handle: Handle<KIND>, key: StringHandle) -> Option<A> {
        let rank = self.rank[handle.index()];
        if rank < 3 {
            return None;
        }
        self.hash_map_ref(rank, self.index[handle.index()]).get(key)
    }

    pub fn put(&mut self, handle: Handle<KIND>, key: StringHandle, value: A) -> bool {
        let rank = self.rank[handle.index()];

        if rank < 3 {
            let map = self.pool_mut(3).alloc();
            self.rank[handle.index()] = 3;
            self.index[handle.index()] = map;
            return self.hash_map_mut(3, map).put(key, value);
        }

        let index = self.index[handle.index()];
        if self.hash_map_ref(rank, index).is_full() {
            let map = self.pool_mut(rank + 1).alloc();
            // copy data
            for i in 0..self.pool_ref(rank).capacity {
                let k = self.hash_map_ref(rank, index).keys[i];
                if k.is_valid() {
                    let v = self.hash_map_ref(rank, index).values[i];
                    self.hash_map_mut(rank + 1, map).put(k, v);
                }
            }
            // free
            self.pool_mut(rank).free(index);
            // adjust
            self.rank[handle.index()] += 1;
            self.index[handle.index()] = map;
            return self.hash_map_mut(rank + 1, map).put(key, value);
        }
        self.hash_map_mut(rank, index).put(key, value)
    }

    pub fn delete(&mut self, handle: Handle<KIND>, key: StringHandle) -> bool {
        let rank = self.rank[handle.index()];
        if rank < 3 {
            return false;
        }
        self.hash_map_mut(rank, self.index[handle.index()])
            .delete(key)
    }
}

impl<A: Copy + Default + Traceable, const KIND: usize> Pool<KIND> for HashMaps<A, KIND> {
    fn byte_count(&self) -> usize {
        let mut bc = mem::size_of::<Self>();
        for pool in &self.pools {
            bc += pool.byte_count();
        }
        bc
    }

    fn trace(&mut self, handle: Handle<KIND>, collector: &mut Collector) {
        if !self.handles.mark(handle.0) {
            return;
        }
        let rank = self.rank[handle.index()];
        let index = self.index[handle.index()];
        self.hash_map_ref(rank, index).trace(collector);
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {
        for i in 0..self.rank.len() {
            let rank = self.rank[i];
            if rank >= 3 && !self.handles.is_marked(i as u32) {
                let index = self.index[i];
                self.pool_mut(self.rank[i]).free(index);
                self.rank[i] = 0;
                self.index[i] = 0;
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
