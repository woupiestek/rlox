use crate::{
    loxtr::{hash_str, Loxtr},
    memory::{Handle, GC},
    object::{Closure, Value},
};

#[derive(Clone, Copy, Debug)]
enum Key {
    Empty,
    Taken { name: GC<Loxtr> },
    Tombstone,
}

pub struct Table<V: Clone> {
    count: usize,
    capacity: usize,
    keys: Box<[Key]>,
    values: Box<[Option<V>]>,
}

impl<V: Clone> Table<V> {
    const MAX_LOAD: f64 = 0.75;
    pub fn new() -> Self {
        Self {
            count: 0,
            capacity: 0,
            keys: Box::from([]),
            values: Box::from([]),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    fn find(keys: &[Key], mask: usize, key: GC<Loxtr>) -> usize {
        let mut index = key.hash_code() as usize & mask;
        let mut tombstone: Option<usize> = None;
        loop {
            match keys[index] {
                Key::Empty => return tombstone.unwrap_or(index),
                Key::Taken { name } => {
                    if name == key {
                        return index;
                    }
                }
                Key::Tombstone => tombstone = Some(index),
            }
            index = (index + 1) & mask;
        }
    }

    fn grow(&mut self, capacity: usize) {
        let mut keys: Box<[Key]> = vec![Key::Empty; capacity].into_boxed_slice();
        let mut values: Box<[Option<V>]> = vec![None; capacity].into_boxed_slice();
        let mask = capacity - 1;
        self.count = 0;
        for i in 0..self.keys.len() {
            if let Key::Taken { name } = self.keys[i] {
                let j = Self::find(&keys, mask, name);
                keys[j] = self.keys[i];
                values[j] = self.values[i].clone();
                self.count += 1;
            }
        }
        self.keys = keys;
        self.values = values;
        self.capacity = capacity;
    }

    pub fn get(&self, key: GC<Loxtr>) -> Option<V> {
        if self.count == 0 {
            return None;
        }
        match &self.values[Self::find(&self.keys, self.capacity - 1, key)] {
            Some(v) => Some(v.clone()),
            None => None
        }
    }

    pub fn set(&mut self, key: GC<Loxtr>, value: V) -> bool {
        if (self.count + 1) as f64 > (self.capacity as f64) * Self::MAX_LOAD {
            self.grow(if self.capacity < 8 {
                8
            } else {
                self.capacity * 2
            })
        }
        let index = Self::find(&self.keys, self.capacity - 1, key);
        let is_new_key = self.values[index].is_none();
        self.values[index] = Some(value);
        if is_new_key {
            self.keys[index] = Key::Taken { name: key };
            self.count += 1;
        }
        is_new_key
    }

    pub fn delete(&mut self, key: GC<Loxtr>) -> bool {
        if self.count == 0 {
            return false;
        }
        let index = Self::find(&self.keys, self.capacity - 1, key);
        if self.values[index].is_none() {
            return false;
        }
        self.keys[index] = Key::Tombstone;
        self.values[index] = None;
        true
    }

    pub fn set_all(&mut self, other: &Table<V>) {
        if self.capacity < other.capacity {
            self.grow(other.capacity)
        }
        for i in 0..other.keys.len() {
            if let Key::Taken { name } = other.keys[i] {
                if let Some(v) = &other.values[i] {
                    self.set(name, v.clone());
                }
            }
        }
    }
}

impl Table<GC<Closure>> {
    // trace: and keys have no properties to trace
    pub fn trace(&self, collector: &mut Vec<Handle>) {
        for value in self.values.iter() {
            if let Some(value) = value {
                collector.push(Handle::from(*value))
            }
        }
    }
}
impl Table<Value> {
    // trace: and keys have no properties to trace
    pub fn trace(&self, collector: &mut Vec<Handle>) {
        for value in self.values.iter() {
            if let Some(Value::Object(handle)) = value {
                collector.push(*handle)
            }
        }
    }
}

impl Table<()> {
    pub fn sweep(&mut self) {
        for index in 0..self.capacity {
            if let Key::Taken { name } = self.keys[index] {
                if !name.is_marked() {
                    self.keys[index] = Key::Tombstone;
                    self.values[index] = None;
                }
            }
        }
    }

    pub fn find_key(&self, str: &str) -> Option<GC<Loxtr>> {
        let hash = hash_str(str);
        if self.count == 0 {
            return None;
        }
        let mask = self.capacity - 1;
        let mut index = hash as usize & mask;
        loop {
            match self.keys[index] {
                Key::Empty => return None,
                Key::Taken { name } => {
                    if name.as_ref() == str {
                        return Some(name);
                    }
                }
                Key::Tombstone => (),
            }
            index = (index + 1) & mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::memory::{Heap, Kind};

    use super::*;

    #[test]
    pub fn set_and_get() {
        let mut heap = Heap::new();
        let mut table = Table::new();
        let key = heap.intern_copy("name");
        let handle = Handle::from(key);
        assert_eq!(handle.kind(), Kind::String);
        assert_eq!(key.is_marked(), false);
        assert_eq!(key.as_ref(), "name");
        assert!(table.set(key, ()));
        assert!(table.get(key).is_some());
    }
}
