use std::mem;

use crate::{
    handles::HandleSet,
    heap::{Collector, Pool, Traceable},
    symbols::SymbolHandle,
};

// all this complexity was needed because of the byte counts.

struct HashMap<A> {
    indices: Box<[u16]>, // limits classes and objects to 65536 members, which seems reasonable.
    keys: Vec<SymbolHandle>, // maybe 65536 is enough identifiers for any program, altough it could be surpassed by generated code and lots of libraries.
    values: Vec<A>,
}

impl<A> HashMap<A> {
    // note: no sacrifical symbols needed anymore.
    const EMPTY: u16 = u16::MAX;

    fn find(&self, key: SymbolHandle) -> (bool, usize) {
        let mask = self.indices.len() - 1;
        let mut hash = (key.0.reverse_bits() >> (mask as u32).leading_zeros()) as usize;
        loop {
            if self.indices[hash] == Self::EMPTY {
                return (false, hash);
            }
            let other = self.indices[hash];
            if self.keys[other as usize] == key {
                return (true, hash);
            }
            hash = (hash + 1) & mask;
        }
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

    pub fn get_ref(&self, key: SymbolHandle) -> Option<&A> {
        let (matched, hash) = self.find(key);
        if matched {
            Some(&self.values[self.indices[hash] as usize])
        } else {
            None
        }
    }

    fn grow(&mut self, lower_bound: usize) -> usize {
        if lower_bound <= self.indices.len() {
            return 0;
        }
        let new_len = lower_bound.next_power_of_two();
        let byte_count = (new_len - self.indices.len()) * mem::size_of::<u16>();
        self.indices = vec![Self::EMPTY; new_len].into_boxed_slice();
        for i in 0..self.keys.len() {
            let key = self.keys[i];
            let (_, hash) = self.find(key);
            self.indices[hash] = i as u16;
        }
        byte_count
    }

    // has a key been added, and if so, how much more space was allocated?
    pub fn put(&mut self, key: SymbolHandle, value: A) -> Option<usize> {
        let (matched, hash) = self.find(key);
        if matched {
            self.values[self.indices[hash] as usize] = value;
            return None;
        }
        // vecs are great, but I need to count the bytes...
        let mut byte_count = 0;
        if self.keys.capacity() == self.keys.len() {
            // fair assumption?
            byte_count +=
                self.keys.capacity() * (mem::size_of::<SymbolHandle>() + mem::size_of::<A>());
        }
        self.indices[hash] = self.keys.len() as u16;
        self.keys.push(key);
        self.values.push(value);
        byte_count += self.grow(self.keys.len() * 4 / 3);
        return Some(byte_count);
    }

    pub fn reserve(&mut self, additional: usize) -> usize {
        let byte_count_before = self.byte_count();
        self.keys.reserve(additional);
        self.values.reserve(additional);
        self.byte_count() - byte_count_before
    }

    pub fn size(&self) -> usize {
        self.keys.len()
    }

    pub fn byte_count(&self) -> usize {
        mem::size_of::<Self>()
            + self.indices.len() * mem::size_of::<u16>()
            + self.keys.capacity() * mem::size_of::<SymbolHandle>()
            + self.values.capacity() * mem::size_of::<A>()
    }
}

impl<A: Traceable> Traceable for HashMap<A> {
    fn trace(&self, collector: &mut Collector) {
        for key in &self.keys {
            key.trace(collector);
        }
        for value in &self.values {
            value.trace(collector);
        }
    }
}

pub struct HashMaps<A> {
    handles: HandleSet,
    active: Vec<Option<HashMap<A>>>,
    hash_map_byte_count: usize,
}

impl<A> HashMaps<A> {
    pub fn new() -> Self {
        Self {
            handles: HandleSet::new(),
            active: Vec::new(),
            hash_map_byte_count: 0,
        }
    }

    pub fn new_hash_map(&mut self) -> u32 {
        let next = self.handles.next();
        if self.active.len() <= next as usize {
            let new_len = (next as usize + 1).next_power_of_two();
            self.active.resize_with(new_len, || None);
        }
        next
    }

    pub fn get_ref(&self, handle: u32, key: SymbolHandle) -> Option<&A> {
        if let Some(hash_map) = &self.active[handle as usize] {
            hash_map.get_ref(key)
        } else {
            None
        }
    }

    pub fn put(&mut self, handle: u32, key: SymbolHandle, value: A) -> bool {
        if let Some(hash_map) = &mut self.active[handle as usize] {
            if let Some(byte_count) = hash_map.put(key, value) {
                self.hash_map_byte_count += byte_count;
                true
            } else {
                false
            }
        } else {
            let mut hash_map = HashMap::with_capacity(8);
            hash_map.put(key, value);
            self.hash_map_byte_count += hash_map.byte_count();
            self.active[handle as usize] = Some(hash_map);
            true
        }
    }
}

impl<A: Copy> HashMaps<A> {
    pub fn add_all(&mut self, source: u32, target: u32) {
        // figures: cannot borrow self.active twice, even if the borrows are disjoint.
        let size = if let Some(map) = self.active[source as usize].as_ref() {
            map.size()
        } else {
            return;
        };
        self.active[target as usize]
            .get_or_insert_with(|| HashMap::with_capacity(8))
            .reserve(size);
        for i in 0..size {
            let k = self.active[source as usize].as_ref().unwrap().keys[i];
            let v = self.active[source as usize].as_ref().unwrap().values[i];
            self.active[target as usize].as_mut().unwrap().put(k, v);
        }
    }
}

pub struct HashMapPool<A: Traceable, const KIND: usize> {
    pub maps: HashMaps<A>,
}

impl<A: Traceable, const KIND: usize> HashMapPool<A, KIND> {
    pub fn new() -> Self {
        Self {
            maps: HashMaps::new(),
        }
    }
}

impl<A: Traceable, const KIND: usize> Pool<KIND> for HashMapPool<A, KIND> {
    fn byte_count(&self) -> usize {
        mem::size_of::<Self>()
            + self.maps.active.capacity() * mem::size_of::<Option<HashMap<A>>>()
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
                self.maps.active[i] = None;
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
        assert!(properties.put(key, Value::TRUE).is_some());
        assert_eq!(Some(Value::TRUE), properties.get_ref(key).copied());
        assert_eq!(None, properties.get_ref(key2).copied());
    }
}
