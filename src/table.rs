use crate::{
    bitarray::BitArray,
    heap::{Handle, Heap},
    loxtr::{hash_str, Loxtr},
    object::Value,
};

#[derive(Clone, Copy, Debug)]
enum Key {
    Empty,
    Taken { name: Handle },
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

    fn find(keys: &[Key], mask: usize, key: Handle, heap: &Heap) -> usize {
        let mut index = heap.get_ref::<Loxtr>(key).hash_code() as usize & mask;
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

    fn grow(&mut self, capacity: usize, heap: &Heap) {
        let mut keys: Box<[Key]> = vec![Key::Empty; capacity].into_boxed_slice();
        let mut values: Box<[Option<V>]> = vec![None; capacity].into_boxed_slice();        
        let mask = capacity - 1;
        self.count = 0;
        for i in 0..self.keys.len() {
            if let Key::Taken { name } = self.keys[i] {
                let j = Self::find(&keys, mask, name, heap);
                keys[j] = self.keys[i];
                values[j] = self.values[i].clone();
                self.count += 1;
            }
        }
        self.keys = keys;
        self.values = values;
        self.capacity = capacity;
    }

    pub fn get(&self, key: Handle, heap: &Heap) -> Option<V> {
        if self.count == 0 {
            return None;
        }
        match &self.values[Self::find(&self.keys, self.capacity - 1, key, heap)] {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }

    pub fn set(&mut self, key: Handle, value: V, heap: &Heap) -> bool {
        if (self.count + 1) as f64 > (self.capacity as f64) * Self::MAX_LOAD {
            self.grow(
                if self.capacity < 8 {
                    8
                } else {
                    self.capacity * 2
                },
                heap,
            )
        }
        let index = Self::find(&self.keys, self.capacity - 1, key, heap);
        let is_new_key = self.values[index].is_none();
        self.values[index] = Some(value);
        if is_new_key {
            self.keys[index] = Key::Taken { name: key };
            self.count += 1;
        }
        is_new_key
    }

    pub fn delete(&mut self, key: Handle, heap: &Heap) -> bool {
        if self.count == 0 {
            return false;
        }
        let index = Self::find(&self.keys, self.capacity - 1, key, heap);
        if self.values[index].is_none() {
            return false;
        }
        self.keys[index] = Key::Tombstone;
        self.values[index] = None;
        true
    }

    pub fn set_all(&mut self, other: &Table<V>, heap: &Heap) {
        if self.capacity < other.capacity {
            self.grow(other.capacity, heap)
        }
        for i in 0..other.keys.len() {
            if let Key::Taken { name } = other.keys[i] {
                if let Some(v) = &other.values[i] {
                    self.set(name, v.clone(), heap);
                }
            }
        }
    }
}

impl Table<Handle> {
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
    pub fn sweep(&mut self, marked: BitArray) {
        for index in 0..self.capacity {
            if let Key::Taken { name: _ } = self.keys[index] {
                if !marked.get(index) {
                    self.keys[index] = Key::Tombstone;
                    self.values[index] = None;
                }
            }
        }
    }

    pub fn add_str(&mut self, str: &str, heap: &mut Heap) -> Handle {
        if (self.count + 1) as f64 > (self.capacity as f64) * Self::MAX_LOAD {
            self.grow(
                if self.capacity < 8 {
                    8
                } else {
                    self.capacity * 2
                },
                heap,
            )
        }
        let hash = hash_str(str);
        let mask = self.capacity - 1;
        let mut index = hash as usize & mask;
        loop {
            match self.keys[index] {
                Key::Empty => {
                    let name = heap.put(Loxtr::copy(str));
                    self.keys[index] = Key::Taken { name };
                    return name;
                }
                Key::Taken { name } => {
                    if heap.get_ref::<Loxtr>(name).as_ref() == str {
                        return name;
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
    use crate::heap::{Heap, Kind};

    use super::*;

    #[test]
    pub fn set_and_get() {
        let mut heap = Heap::new();
        let mut table = Table::new();
        let key = heap.intern_copy("name");
        let handle = Handle::from(key);
        assert_eq!(heap.kind(handle), Kind::String);
        assert_eq!(heap.get_ref::<Loxtr>(handle).as_ref(), "name");
        assert!(table.set(key, (), &heap));
        assert!(table.get(key, &heap).is_some());
    }
}
