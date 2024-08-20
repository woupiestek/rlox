use std::u32;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StringHandle(u32);

impl StringHandle {
    pub const EMPTY: Self = Self(0);
    pub const TOMBSTONE: Self = Self(u32::MAX);
    pub fn is_valid(&self) -> bool {
        self != &StringHandle::EMPTY && self != &StringHandle::TOMBSTONE
    }
}

struct KeySet {
    count: usize,
    keys: Box<[StringHandle]>,
}

impl KeySet {
    const MAX_LOAD: f64 = 0.75;

    pub fn with_capacity(capacity: usize) -> Self {
        assert!(
            capacity == 0 || capacity.is_power_of_two(),
            "capacity should be zero or power of two"
        );
        Self {
            count: 0,
            keys: vec![StringHandle::EMPTY; capacity].into_boxed_slice(),
        }
    }

    fn find(&self, key: StringHandle) -> (bool, usize) {
        assert!(self.keys.len() > 0 && 4 * self.count <= 3 * self.keys.len());
        let keys: &[StringHandle] = &self.keys;
        let mask = keys.len() - 1;
        let mut index = key.0 as usize & mask;
        let mut tombstone: Option<usize> = None;
        loop {
            match keys[index] {
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

    fn add(&mut self, key: StringHandle) -> usize {
        let (found, index) = self.find(key);
        if !found {
            self.keys[index] = key;
            self.count += 1;
        }
        index
    }

    // keyset in map needs to say when a value can be evicted.
    fn delete(&mut self, key: StringHandle) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let (found, index) = self.find(key);
        if found {
            self.keys[index] = StringHandle::TOMBSTONE;
            Some(index)
        } else {
            None
        }
    }
}

pub struct Map<V: Clone> {
    key_set: KeySet,
    values: Box<[Option<V>]>,
}

impl<V: Clone> Map<V> {
    pub fn new() -> Self {
        Self {
            key_set: KeySet::with_capacity(0),
            values: Box::from([]),
        }
    }

    pub fn get(&self, key: StringHandle) -> Option<V> {
        let (found, index) = self.key_set.find(key);
        if found {
            self.values[index].clone()
        } else {
            None
        }
    }

    fn capacity(&self) -> usize {
        self.key_set.keys.len()
    }

    fn grow(&mut self, capacity: usize) {
        let mut key_set = KeySet::with_capacity(capacity);
        let mut values: Box<[Option<V>]> = vec![None; capacity].into_boxed_slice();
        for i in 0..self.capacity() {
            let key = self.key_set.keys[i];
            if key.is_valid() {
                values[key_set.add(key)] = self.values[i].take();
            }
        }
        self.key_set = key_set;
        self.values = values;
    }

    pub fn set(&mut self, key: StringHandle, value: V) {
        // grow if necessary
        let capacity = self.capacity();
        if 4 * (self.key_set.count + 1) > 3 * capacity {
            self.grow(if capacity < 8 { 8 } else { 2 * capacity });
        }
        self.values[self.key_set.add(key)] = Some(value);
    }

    pub fn delete(&mut self, key: StringHandle) {
        if let Some(index) = self.key_set.delete(key) {
            self.values[index] = None;
        }
    }

    pub fn set_all(&mut self, other: Self) {
        if self.capacity() < other.capacity() {
            self.grow(other.capacity())
        }
        for i in 0..other.capacity() {
            let key = other.key_set.keys[i];
            if key.is_valid() {
                if let Some(v) = &other.values[i] {
                    self.set(key, v.clone());
                }
            }
        }
    }
}

pub struct Strings {
    key_set: KeySet,
    // marked: KeySet,
    values: Box<[Option<Box<str>>]>,
}

impl Strings {
    pub fn with_capacity(capacity: usize) -> Strings {
        Strings {
            key_set: KeySet::with_capacity(capacity),
            values: vec![None; capacity].into_boxed_slice(),
        }
    }

    fn hash(str: &str) -> u32 {
        let mut hash = 2166136261u32;
        for &byte in str.as_bytes() {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(16777619u32);
        }
        hash
    }

    fn intern(&mut self, str: &str) -> StringHandle {
        let mut index = Self::hash(str);
        if index == 0 || index == u32::MAX {
            index = 1;
        }
        let mask = self.key_set.keys.len() - 1;
        let mut tombstone: Option<u32> = None;
        loop {
            let key = self.key_set.keys[index as usize & mask];
            if key == StringHandle::EMPTY {
                let j = tombstone.unwrap_or(index);
                let handle = StringHandle(j);
                self.key_set.keys[j as usize & mask] = handle;
                self.key_set.count += 1;
                self.values[j as usize & mask] = Some(Box::from(str));
                return handle;
            }
            if key == StringHandle::TOMBSTONE {
                tombstone = Some(index);
                continue;
            }
            if key.0 == index {
                if let Some(x) = &self.values[index as usize & mask] {
                    if x.as_ref() == str {
                        return key;
                    }
                }
            }
            index += 1;
            if index == u32::MAX {
                index = 1;
            }
        }
    }

    fn grow(&mut self, capacity: usize) {
        let mut key_set = KeySet::with_capacity(capacity);
        let mut values: Box<[Option<Box<str>>]> = vec![None; capacity].into_boxed_slice();
        for i in 0..self.key_set.keys.len() {
            let key = self.key_set.keys[i];
            if key.is_valid() {
                values[key_set.add(key)] = self.values[i].take();
            }
        }
        self.key_set = key_set;
        self.values = values;
    }

    fn grow_if_necessary(&mut self) {
        if 4 * (self.key_set.count + 1) <= 3 * self.key_set.keys.len() {
            return;
        }
        self.grow(2 * self.key_set.keys.len())
    }

    pub fn store(&mut self, str: &str) -> StringHandle {
        self.grow_if_necessary();
        self.intern(str)
    }

    pub fn deref(&self, handle: StringHandle) -> Option<&str> {
        let (found, index) = self.key_set.find(handle);
        if found {
            self.values[index].as_deref()
        } else {
            None
        }
    }

    // I still imagine that for sweep, we just create a new keyset (mark) and use that to replace the keyset here (sweep)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn string_equality() {
        let mut strings = Strings::with_capacity(8);
        let key = strings.store("str");
        assert_eq!(key, strings.store("str"));
        assert_eq!(Some("str"), strings.deref(key));

        let key1 = strings.store("one");
        let key2 = strings.store("two");
        assert_eq!(key2, strings.store("two"));
        assert_eq!(Some("one"), strings.deref(key1));
        assert_ne!(key1, key2);
    }

    #[test]
    pub fn growth() {
        let mut strings = Strings::with_capacity(8);
        let mut handles: Vec<StringHandle> = Vec::new();
        let mut values: Vec<String> = Vec::new();
        for i in 0..12 {
            let value = "str".to_owned() + &i.to_string();
            handles.push(strings.store(&value));
            values.push(value.clone());
        }
        for i in 0..12 {
            assert_eq!(Some(values[i].as_str()), strings.deref(handles[i]));
        }
    }

    #[test]
    pub fn set_and_get() {
        let mut strings = Strings::with_capacity(8);
        let mut table = Map::new();
        let key = strings.store("name");
        table.set(key, ());
        assert_eq!(Some(()), table.get(key));
    }
}
