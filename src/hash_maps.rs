use std::mem;

use crate::{
    handles::HandleSet,
    heap::{Collector, Pool, Traceable},
    symbols::SymbolHandle,
};

/*
 * Purpose:
 * - reuse of logic at least between globals, classes and instances
 * - never give any memory up, but reuse aggressively instead.
 */

const KNUTH_PHI: u32 = 2654435761;

struct HashMap<A: Copy> {
    indices: Box<[u16]>, // limits classes and objects to 65536 members, which seems reasonable.
    keys: Vec<SymbolHandle>, // maybe 65536 is enough identifiers for any program, altough it could be surpassed by generated code and lots of libraries.
    values: Vec<A>,
}

impl<A: Copy> HashMap<A> {
    // note: no sacrifical symbols needed anymore.
    const EMPTY: u16 = u16::MAX;

    fn find(&self, key: SymbolHandle) -> (bool, usize) {
        let mask = self.indices.len() - 1;
        let mut hash = (key.0.wrapping_mul(KNUTH_PHI) >> (mask as u32).leading_zeros()) as usize;
        loop {
            match self.indices[hash] {
                Self::EMPTY => {
                    return (false, hash);
                }
                other => {
                    if self.keys[other as usize] == key {
                        return (true, hash);
                    }
                }
            }
            hash = (hash + 1) & mask;
        }
    }

    fn is_full(&self) -> bool {
        4 * self.keys.len() > 3 * self.indices.len()
    }

    // based on the previous reuse idea.
    // do we keep working this way?
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity == 0 || capacity.is_power_of_two());
        Self {
            indices: vec![Self::EMPTY; capacity].into_boxed_slice(),
            keys: Vec::with_capacity(capacity * 3 / 4),
            values: Vec::with_capacity(capacity * 3 / 4),
        }
    }

    pub fn get(&self, key: SymbolHandle) -> Option<A> {
        let (matched, hash) = self.find(key);
        if matched {
            Some(self.values[self.indices[hash] as usize])
        } else {
            None
        }
    }

    // is this return value actually used?
    fn put_unchecked(&mut self, key: SymbolHandle, value: A) -> bool {
        let (matched, hash) = self.find(key);
        if matched {
            self.values[self.indices[hash] as usize] = value;
            return false;
        }
        self.indices[hash] = self.keys.len() as u16;
        self.keys.push(key);
        self.values.push(value);
        return true;
    }

    pub fn put(&mut self, key: SymbolHandle, value: A) -> bool {
        // maybe grow? how?
        assert!(!self.is_full());
        self.put_unchecked(key, value)
    }

    pub fn clear(&mut self) {
        self.indices.fill(Self::EMPTY);
        self.keys.clear();
        self.values.clear();
    }

    fn capacity(&self) -> usize {
        self.indices.len()
    }
}

impl<A: Copy + Traceable> Traceable for HashMap<A> {
    fn trace(&self, collector: &mut Collector) {
        for key in &self.keys {
            key.trace(collector);
        }
        for value in &self.values {
            value.trace(collector);
        }
    }
}

pub struct HashMaps<A: Copy> {
    handles: HandleSet,
    active: Vec<Option<HashMap<A>>>,
    stash: Vec<Vec<HashMap<A>>>,
    hash_map_byte_count: usize,
}

impl<A: Copy> HashMaps<A> {
    pub fn new() -> Self {
        Self {
            handles: HandleSet::new(),
            active: Vec::new(),
            stash: Vec::new(),
            hash_map_byte_count: 0,
        }
    }

    pub fn new_hash_map(&mut self) -> u32 {
        let next = self.handles.next();
        while self.active.len() <= next as usize {
            self.active.push(None);
        }
        next
    }

    pub fn get(&self, handle: u32, key: SymbolHandle) -> Option<A> {
        if let Some(hash_map) = &self.active[handle as usize] {
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

    fn resize(&mut self, handle: u32, capacity: usize) {
        if let Some(hash_map) = &self.active[handle as usize] {
            if hash_map.capacity() >= capacity {
                return;
            }
        }
        let mut new_map = self.alloc(capacity);
        let old_map = self.active[handle as usize].take(); //.replace(new_map);
        if let Some(hash_map) = old_map {
            for i in 0..hash_map.keys.len() {
                let k = hash_map.keys[i];
                let v = hash_map.values[i];
                new_map.put(k, v);
            }
            self.stash(hash_map);
        }
        self.active[handle as usize] = Some(new_map);
    }

    fn hash_map_ref(&mut self, handle: u32) -> &HashMap<A> {
        self.active[handle as usize].as_ref().unwrap()
    }

    fn hash_map_mut(&mut self, handle: u32) -> &mut HashMap<A> {
        self.active[handle as usize].as_mut().unwrap()
    }

    pub fn put(&mut self, handle: u32, key: SymbolHandle, value: A) -> bool {
        let capacity = if let Some(hash_map) = &mut self.active[handle as usize] {
            if !hash_map.is_full() {
                return hash_map.put(key, value);
            }
            hash_map.indices.len() * 2
        } else {
            8
        };
        self.resize(handle, capacity);
        self.hash_map_mut(handle).put(key, value)
    }

    pub fn count(&self, handle: u32) -> usize {
        if let Some(hash_map) = &self.active[handle as usize] {
            hash_map.keys.len()
        } else {
            0
        }
    }

    pub fn add_all(&mut self, source: u32, target: u32) {
        if self.active[source as usize].is_none() {
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
}

pub struct HashMapPool<A: Copy + Traceable, const KIND: usize> {
    pub maps: HashMaps<A>,
}

impl<A: Copy + Traceable, const KIND: usize> HashMapPool<A, KIND> {
    pub fn new() -> Self {
        Self {
            maps: HashMaps::new(),
        }
    }
}

impl<A: Copy + Traceable, const KIND: usize> Pool<KIND> for HashMapPool<A, KIND> {
    fn byte_count(&self) -> usize {
        mem::size_of::<Self>()
            + self.maps.active.capacity() * mem::size_of::<Option<HashMap<A>>>()
            + self.maps.stash.capacity() * mem::size_of::<Vec<HashMap<A>>>()
            + self.maps.hash_map_byte_count
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.maps.handles.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        for &i in marked {
            if let Some(hash_map) = &self.maps.active[i as usize] {
                hash_map.trace(collector);
            }
        }
    }

    fn reset(&mut self) {
        self.maps.handles.clear();
    }

    fn sweep(&mut self) {
        for i in 0..self.maps.active.len() {
            if !self.maps.handles.is_marked(i as u32) {
                if let Some(hash_map) = self.maps.active[i].take() {
                    self.maps.stash(hash_map);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{heap::Handle, values::Value};

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
