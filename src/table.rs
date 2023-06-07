use crate::{
    loxtr::{hash_str, Loxtr},
    memory::{Handle, GC},
    object::{Closure, Value},
};

#[derive(Clone, Debug)]
enum Entry<V: Clone> {
    Empty,
    Taken { key: GC<Loxtr>, value: V },
    Tombstone,
}

impl<V: Clone> Entry<V> {
    fn is_empty(&self) -> bool {
        matches!(self, Self::Empty | Self::Tombstone)
    }
}

pub struct Table<V: Clone> {
    count: usize,
    capacity: usize,
    entries: Box<[Entry<V>]>,
}

impl<V: Clone> Table<V> {
    const MAX_LOAD: f64 = 0.75;
    pub fn new() -> Self {
        Self {
            count: 0,
            capacity: 0,
            entries: Box::from([]),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    fn find(entries: &[Entry<V>], mask: usize, key: GC<Loxtr>) -> usize {
        let mut index = key.hash_code() as usize & mask;
        let mut tombstone: Option<usize> = None;
        loop {
            match entries[index] {
                Entry::Empty => return tombstone.unwrap_or(index),
                Entry::Taken { key: k, value: _ } => {
                    if k == key {
                        return index;
                    }
                }
                Entry::Tombstone => tombstone = Some(index),
            }
            index = (index + 1) & mask;
        }
    }

    fn grow(&mut self, capacity: usize) {
        let mut entries: Box<[Entry<V>]> = vec![Entry::Empty; capacity].into_boxed_slice();
        let mask = capacity - 1;
        self.count = 0;
        for entry in self.entries.iter() {
            if let Entry::Taken { key, value: _ } = entry {
                entries[Self::find(&entries, mask, *key)] = entry.clone();
                self.count += 1;
            }
        }
        self.entries = entries;
        self.capacity = capacity;
    }

    pub fn get(&self, key: GC<Loxtr>) -> Option<V> {
        if self.count == 0 {
            return None;
        }
        if let Entry::Taken { key: _, value } =
            &self.entries[Self::find(&self.entries, self.capacity - 1, key)]
        {
            Some(value.clone())
        } else {
            None
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
        let index = Self::find(&self.entries, self.capacity - 1, key);
        let is_new_key = self.entries[index].is_empty();
        if is_new_key {
            self.count += 1;
        }
        self.entries[index] = Entry::Taken { key, value };
        is_new_key
    }

    pub fn delete(&mut self, key: GC<Loxtr>) -> bool {
        if self.count == 0 {
            return false;
        }
        let index = Self::find(&self.entries, self.capacity - 1, key);
        let key_existed = !self.entries[index].is_empty();
        self.entries[index] = Entry::Tombstone;
        key_existed
    }

    pub fn set_all(&mut self, other: &Table<V>) {
        if self.capacity < other.capacity {
            self.grow(other.capacity)
        }
        for entry in other.entries.iter() {
            if let Entry::Taken { key, value } = entry {
                self.set(*key, value.clone());
            }
        }
    }
}

impl Table<GC<Closure>> {
    pub fn trace(&self, collector: &mut Vec<Handle>) {
        for entry in self.entries.iter() {
            if let Entry::Taken { key: _, value } = entry {
                collector.push(Handle::from(*value))
            }
        }
    }
}
impl Table<Value> {
    pub fn trace(&self, collector: &mut Vec<Handle>) {
        for entry in self.entries.iter() {
            if let Entry::Taken {
                key: _,
                value: Value::Object(handle),
            } = entry
            {
                collector.push(*handle)
            }
        }
    }
}

impl Table<()> {
    pub fn sweep(&mut self) {
        for index in 0..self.capacity {
            if let Entry::Taken { key, value: _ } = self.entries[index] {
                if !key.is_marked() {
                    self.entries[index] = Entry::Tombstone
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
            match self.entries[index] {
                Entry::Empty => return None,
                Entry::Taken { key, value: _ } => {
                    if key.as_ref() == str {
                        return Some(key);
                    }
                }
                Entry::Tombstone => (),
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
        let key = heap.intern("name");
        let handle = Handle::from(key);
        assert_eq!(handle.kind(), Kind::String);
        assert_eq!(key.is_marked(), false);
        assert_eq!(key.as_ref(), "name");
        assert!(table.set(key, ()));
        assert!(table.get(key).is_some());
    }
}
