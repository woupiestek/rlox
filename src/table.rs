use crate::{loxtr::Loxtr, memory::Obj};

#[derive(Clone)]
enum Entry<V: Clone> {
    Empty,
    Taken { key: Obj<Loxtr>, value: V },
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

    fn find(entries: &[Entry<V>], mask: usize, key: Obj<Loxtr>) -> usize {
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

    pub fn get(&self, key: Obj<Loxtr>) -> Option<V> {
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

    // return optionally evicted value
    pub fn set(&mut self, key: Obj<Loxtr>, value: V) -> bool {
        if (self.count + 1) as f64 > (self.capacity as f64) * Self::MAX_LOAD {
            self.grow(if self.capacity < 8 {
                8
            } else {
                self.capacity * 2
            })
        }
        let index = Self::find(&self.entries, self.capacity - 1, key);
        let is_new_key = self.entries[index].is_empty();
        self.entries[index] = Entry::Taken { key, value };
        is_new_key
    }

    pub fn delete(&mut self, key: Obj<Loxtr>) -> bool {
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

    pub fn find_key(&self, str: &str, hash: u64) -> Option<Obj<Loxtr>> {
        if self.count == 0 {
            return None;
        }
        let mask = self.capacity - 1;
        let mut index = hash as usize & mask;
        loop {
            match self.entries[index] {
                Entry::Empty => return None,
                Entry::Taken { key, value: _ } => {
                    if key.hash_code() == hash && key.as_ref() == str {
                        return Some(key);
                    }
                }
                Entry::Tombstone => todo!(),
            }
            index = (index + 1) & mask;
        }
    }
}
