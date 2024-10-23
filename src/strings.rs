use std::{mem, u32};

use crate::{
    bitarray::BitArray,
    closures2::ClosureHandle,
    heap::{Collector, Handle, Pool, STRING},
    values::Value,
};

// deliberately distinct
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct StringHandle(pub u32);

impl StringHandle {
    pub const EMPTY: Self = Self(0);
    pub const TOMBSTONE: Self = Self(u32::MAX);
    pub fn is_valid(&self) -> bool {
        self != &StringHandle::EMPTY && self != &StringHandle::TOMBSTONE
    }
}

struct KeySet {
    count: usize,
    keys: Option<Box<[StringHandle]>>,
}

impl KeySet {
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(
            capacity == 0 || capacity.is_power_of_two(),
            "capacity should be zero or power of two"
        );
        Self {
            count: 0,
            keys: if capacity == 0 {
                None
            } else {
                Some(vec![StringHandle::EMPTY; capacity].into_boxed_slice())
            },
        }
    }

    fn capacity(&self) -> usize {
        if let Some(keys) = &self.keys {
            keys.len()
        } else {
            0
        }
    }

    fn get(&self, index: usize) -> StringHandle {
        if let Some(keys) = &self.keys {
            keys[index]
        } else {
            StringHandle::EMPTY
        }
    }

    fn put(&mut self, index: usize, handle: StringHandle) {
        if let Some(keys) = &mut self.keys {
            keys[index] = handle;
        }
    }

    fn find(&self, key: StringHandle) -> (bool, usize) {
        assert!(self.keys.is_some() && 4 * self.count <= 3 * self.capacity());
        let mask = self.capacity() - 1;
        let mut index = key.0 as usize & mask;
        let mut tombstone: Option<usize> = None;
        loop {
            match self.get(index) {
                StringHandle::EMPTY => return (false, tombstone.unwrap_or(index)),
                StringHandle::TOMBSTONE => tombstone = Some(index),
                ki => {
                    if ki == key {
                        return (true, index);
                    }
                }
            }
            index = (index + 1) & mask;
        }
    }

    fn add(&mut self, key: StringHandle) -> (bool, usize) {
        let (found, index) = self.find(key);
        if !found {
            self.put(index, key);
            self.count += 1;
        }
        (found, index)
    }

    // keyset in map need to say when a value can be evicted.
    fn delete(&mut self, key: StringHandle) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let (found, index) = self.find(key);
        if found {
            self.put(index, StringHandle::TOMBSTONE);
            Some(index)
        } else {
            None
        }
    }
}

pub struct Map<V: Copy + Default> {
    key_set: KeySet,
    values: Option<Box<[V]>>,
}

impl<V: Copy + Default> Map<V> {
    pub fn new() -> Self {
        Self {
            key_set: KeySet::with_capacity(0),
            values: None,
        }
    }

    pub fn capacity(&self) -> usize {
        if let Some(keys) = &self.key_set.keys {
            keys.len()
        } else {
            0
        }
    }

    fn get_value_by_index(&self, index: usize) -> V {
        if let Some(values) = &self.values {
            values[index]
        } else {
            V::default()
        }
    }

    fn set_value_by_index(&mut self, index: usize, value: V) {
        if let Some(values) = &mut self.values {
            values[index] = value;
        }
    }

    pub fn get(&self, key: StringHandle) -> Option<V> {
        if self.key_set.count == 0 {
            return None;
        }
        let (found, index) = self.key_set.find(key);
        if found {
            Some(self.get_value_by_index(index))
        } else {
            None
        }
    }

    fn grow(&mut self, capacity: usize) {
        assert_ne!(capacity, 0, "what were you thinking?");
        let mut key_set = KeySet::with_capacity(capacity);
        let mut values: Box<[V]> = vec![V::default(); capacity].into_boxed_slice();
        for i in 0..self.capacity() {
            let key = self.key_set.get(i);
            if key.is_valid() {
                values[key_set.add(key).1] = self.get_value_by_index(i);
            }
        }
        self.key_set = key_set;
        self.values = Some(values);
    }

    // returns true if a value is overridden
    pub fn set(&mut self, key: StringHandle, value: V) -> bool {
        // grow if necessary
        let capacity = self.capacity();
        if 4 * (self.key_set.count + 1) > 3 * capacity {
            self.grow(if capacity < 8 { 8 } else { 2 * capacity });
        }
        let (found, index) = self.key_set.add(key);
        self.set_value_by_index(index, value);
        return found;
    }

    pub fn delete(&mut self, key: StringHandle) {
        if let Some(index) = self.key_set.delete(key) {
            self.set_value_by_index(index, V::default());
        }
    }

    #[cfg(feature = "trace")]
    pub fn keys(&self) -> KeyIterator {
        KeyIterator {
            key_set: &self.key_set,
            index: self.key_set.keys.len(),
        }
    }
}

impl<V: Copy + Default> Clone for Map<V> {
    fn clone(&self) -> Self {
        let mut clone = Map::new();
        clone.grow(self.capacity());
        for i in 0..self.capacity() {
            let key = self.key_set.get(i);
            if key.is_valid() {
                clone.set(key, self.get_value_by_index(i));
            }
        }
        clone
    }
}

impl Map<ClosureHandle> {
    pub fn trace(&self, collector: &mut Collector) {
        for i in 0..self.capacity() {
            // in case a string get resurrected
            if self.key_set.get(i).is_valid() {
                collector.keys.push(self.key_set.get(i));
                collector.push(self.get_value_by_index(i));
            }
        }
    }
}

impl Map<Value> {
    pub fn trace(&self, collector: &mut Collector) {
        for i in 0..self.capacity() {
            if self.key_set.get(i).is_valid() {
                collector.keys.push(self.key_set.get(i));
                self.get_value_by_index(i).trace(collector);
            }
        }
    }
}

pub struct KeyIterator<'m> {
    key_set: &'m KeySet,
    index: usize,
}

// note type members...
impl<'m> Iterator for KeyIterator<'m> {
    type Item = StringHandle;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index > 0 {
            self.index -= 1;
            let sh = self.key_set.get(self.index);
            if sh.is_valid() {
                return Some(sh);
            }
        }
        return None;
    }
}

pub struct Strings {
    key_set: KeySet,
    generations: Box<[u8]>,
    strs: Box<[Option<Box<str>>]>,
    str_byte_count: usize,
}

impl Strings {
    pub fn with_capacity(capacity: usize) -> Strings {
        Strings {
            key_set: KeySet::with_capacity(capacity),
            strs: vec![None; capacity].into_boxed_slice(),
            generations: vec![0; capacity].into_boxed_slice(),
            str_byte_count: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        if let Some(keys) = &self.key_set.keys {
            keys.len()
        } else {
            0
        }
    }

    // 24 bit hash, which leaves 8 generation bits at the top.
    fn hash(str: &str) -> u32 {
        let mut hash = 2166136261u32;
        for &byte in str.as_bytes() {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(16777619u32);
        }
        (hash >> 24) ^ hash & 0xFFFFFF
    }

    fn grow(&mut self, capacity: usize) {
        let mut key_set = KeySet::with_capacity(capacity);
        let mut values: Box<[Option<Box<str>>]> = vec![None; capacity].into_boxed_slice();
        let mut generations: Box<[u8]> = vec![0; capacity].into_boxed_slice();
        for i in 0..self.key_set.capacity() {
            let key = self.key_set.get(i);
            if key.is_valid() {
                let j = key_set.add(key).1;
                values[j] = self.strs[i].take();
                generations[j] = self.generations[i];
            }
        }
        self.key_set = key_set;
        self.strs = values;
        self.generations = generations;
    }

    fn grow_if_necessary(&mut self) {
        let capacity = self.capacity();
        if 4 * (self.key_set.count + 1) <= 3 * capacity {
            return;
        }
        let capacity = if capacity < 8 {
            8
        } else {
            2 * self.key_set.capacity()
        };
        self.grow(capacity);
        assert_eq!(self.capacity(), capacity);
    }

    pub fn put(&mut self, str: &str) -> StringHandle {
        self.grow_if_necessary();
        let hash = Self::hash(str);
        let mask = self.key_set.capacity() - 1;
        let mut generation: u8 = 0;
        if hash == 0 || hash == u32::MAX {
            generation = 1;
        }
        let mut index = (hash as usize) & mask;
        let mut tombstone: Option<usize> = None;
        loop {
            let key = self.key_set.get(index);
            if key == StringHandle::EMPTY {
                let j = tombstone.unwrap_or(index);
                // combine generations
                let handle = StringHandle(hash ^ ((generation as u32) << 24));
                self.key_set.put(j, handle);
                self.generations[j] = generation;
                self.key_set.count += 1;
                self.str_byte_count += str.len();
                self.strs[j] = Some(Box::from(str));
                return handle;
            }
            if key == StringHandle::TOMBSTONE {
                tombstone = Some(index);
                continue;
            }
            if key.0 & 0xffffff == hash {
                if let Some(x) = &self.strs[index as usize & mask] {
                    if x.as_ref() == str {
                        return key;
                    }
                    let g = self.generations[index];
                    if generation <= g {
                        assert!(g < u8::MAX, "string pool failed: too many hash collisions");
                        generation = g + 1;
                    }
                }
            }
            index += 1;
            index &= mask;
        }
    }

    pub fn get(&self, handle: StringHandle) -> Option<&str> {
        let (found, index) = self.key_set.find(handle);
        if found {
            self.strs[index].as_deref()
        } else {
            None
        }
    }

    pub fn concat(&mut self, a: StringHandle, b: StringHandle) -> Option<StringHandle> {
        if let (Some(a), Some(b)) = (self.get(a), self.get(b)) {
            let mut c = String::new();
            c.push_str(a);
            c.push_str(b);
            Some(self.put(&c))
        } else {
            None
        }
    }

    const ENTRY_SIZE: usize = (mem::size_of::<Option<Box<str>>>() + mem::size_of::<StringHandle>());
}

impl Pool<STRING> for Strings {
    fn byte_count(&self) -> usize {
        self.capacity() * Self::ENTRY_SIZE + self.str_byte_count
    }

    fn count(&self) -> usize {
        self.key_set.count
    }

    fn trace(&self, _handle: Handle<STRING>, _collector: &mut Collector) {}

    fn sweep(&mut self, marks: &BitArray) {
        for i in 0..self.capacity() {
            if !marks.has(i) {
                self.key_set.delete(self.key_set.get(i));
                if let Some(str) = self.strs[i].take() {
                    self.str_byte_count -= str.len();
                }
            }
        }
    }

    fn mark(&self, collector: &mut Collector) -> bool {
        if collector.keys.is_empty() {
            return true;
        }
        while let Some(key) = collector.keys.pop() {
            let (found, i) = self.key_set.find(key);
            if found {
                collector.marks[STRING].add(i);
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn string_equality() {
        let mut strings = Strings::with_capacity(8);
        let key = strings.put("str");
        assert_eq!(key, strings.put("str"));
        assert_eq!(Some("str"), strings.get(key));

        let key1 = strings.put("one");
        let key2 = strings.put("two");
        assert_eq!(key2, strings.put("two"));
        assert_eq!(Some("one"), strings.get(key1));
        assert_ne!(key1, key2);
    }

    #[test]
    pub fn growth() {
        let mut strings = Strings::with_capacity(8);
        let mut handles: Vec<StringHandle> = Vec::new();
        let mut values: Vec<String> = Vec::new();
        for i in 0..12 {
            let value = "str".to_owned() + &i.to_string();
            handles.push(strings.put(&value));
            values.push(value.clone());
        }
        for i in 0..12 {
            assert_eq!(Some(values[i].as_str()), strings.get(handles[i]));
        }
    }

    #[test]
    pub fn set_and_get() {
        let mut strings = Strings::with_capacity(8);
        let mut table = Map::new();
        let key = strings.put("name");
        let key2 = strings.put("other");
        table.set(key, ());
        assert_eq!(Some(()), table.get(key));
        assert_eq!(None, table.get(key2));
    }
}
