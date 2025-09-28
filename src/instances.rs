use std::mem;

use crate::{
    classes::ClassHandle,
    handles::Handles,
    heap::{Collector, Handle, Heap, Pool, Traceable, INSTANCE},
    strings::StringHandle,
    values::Value,
};

pub struct Properties {
    count: usize,
    keys: Box<[StringHandle]>,
    leading_zeros: u32,
    mask: usize,
    values: Box<[Value]>,
}

const KNUTH_PHI: u32 = 2654435761;

// not implemented to grow automatically
// so the garbage collector can count how many bytes were used
impl Properties {
    fn byte_count(&self) -> usize {
        mem::size_of::<Properties>() + self.keys.len() * 12
    }

    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "power of two required");
        let mask = capacity - 1;
        Self {
            count: 0,
            keys: vec![StringHandle::EMPTY; capacity].into_boxed_slice(),
            mask,
            leading_zeros: (mask as u32).leading_zeros(),
            values: vec![Value::NIL; capacity].into_boxed_slice(),
        }
    }

    fn capacity(&self) -> usize {
        self.keys.len()
    }

    fn hash(&self, key: StringHandle) -> usize {
        (key.0.wrapping_mul(KNUTH_PHI) >> self.leading_zeros) as usize
    }

    fn find(&self, key: StringHandle) -> usize {
        let mut index = self.hash(key);
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
            index = (index + 1) & self.mask;
        }
    }

    pub fn get(&self, key: StringHandle) -> Option<Value> {
        let index = self.find(key);
        if self.keys[index] == key {
            Some(self.values[index])
        } else {
            None
        }
    }

    pub fn is_full(&self) -> bool {
        4 * self.count > 3 * self.capacity()
    }

    // true mean a new key was added
    // false means it was not
    // there is no indication of what happened to the value.
    pub fn put(&mut self, key: StringHandle, value: Value) -> bool {
        if self.is_full() {
            return false;
        }
        let index = self.find(key);
        self.values[index] = value;
        if self.keys[index] == key {
            return false;
        }
        self.keys[index] = key;
        self.count += 1;
        true
    }

    pub fn trace(&self, collector: &mut Collector) {
        for index in 0..self.capacity() {
            let key = self.keys[index];
            if !key.is_valid() {
                continue;
            }
            key.trace(collector);
            self.values[index].trace(collector);
        }
    }

    pub fn grow(&mut self) -> Properties {
        let mut new_properties = Properties::with_capacity(self.capacity() * 2);
        for index in 0..self.capacity() {
            let key = self.keys[index];
            if key == StringHandle::EMPTY {
                continue;
            }
            new_properties.put(key, self.values[index]);
        }
        new_properties
    }

    pub fn delete(&mut self, key: StringHandle) {
        let index = self.find(key);
        if self.keys[index] == key {
            self.keys[index] = StringHandle::TOMBSTONE;
            self.values[index] = Value::NIL;
            // not counted. tombstones must be removed
        }
    }
}

pub type InstanceHandle = Handle<INSTANCE>;

pub struct Instances {
    byte_count: usize,
    classes: Vec<ClassHandle>,
    properties: Vec<Properties>,
    handles: Handles,
}

impl Instances {
    pub fn new() -> Self {
        Self {
            byte_count: 56,
            classes: Vec::new(),
            properties: Vec::new(),
            handles: Handles::new(),
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        let index = self.handles.next() as usize;
        if index < self.properties.len() {
            self.byte_count -= self.properties[index].byte_count();
            self.properties[index] = Properties::with_capacity(8);
            self.classes[index] = class;
            self.byte_count += self.properties[index].byte_count();
        } else {
            while index >= self.properties.len() {
                let properties = Properties::with_capacity(8);
                self.byte_count += properties.byte_count();
                self.properties.push(properties);
                self.classes.push(class);
            }
        }

        InstanceHandle::from(index as u32)
    }

    pub fn get_class<'s>(&self, ih: InstanceHandle) -> ClassHandle {
        self.classes[ih.index()]
    }

    pub fn to_string(&self, ih: InstanceHandle, heap: &Heap) -> String {
        format!(
            "<{} instance>",
            heap.classes.get_name(self.get_class(ih), &heap.strings)
        )
    }

    pub fn get_property(&self, ih: InstanceHandle, key: StringHandle) -> Option<Value> {
        self.properties[ih.index()].get(key)
    }

    pub fn set_property(&mut self, ih: InstanceHandle, key: StringHandle, value: Value) {
        if self.properties[ih.index()].is_full() {
            self.byte_count -= self.properties[ih.index()].byte_count();
            self.properties[ih.index()] = self.properties[ih.index()].grow();
            self.byte_count += self.properties[ih.index()].byte_count();
        }
        self.properties[ih.index()].put(key, value);
    }
}

impl Pool<INSTANCE> for Instances {
    fn byte_count(&self) -> usize {
        self.byte_count + self.classes.len() * 4
    }
    fn trace(&mut self, handle: Handle<INSTANCE>, collector: &mut Collector) {
        if !self.handles.mark(handle.0) {
            return;
        }
        self.classes[handle.index()].trace(collector);
        self.properties[handle.index()].trace(collector);
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {}
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn put_and_get() {
        let mut properties = Properties::with_capacity(8);
        let key = Handle(60);
        let key2 = Handle(80);
        assert!(properties.put(key, Value::TRUE));
        assert_eq!(Some(Value::TRUE), properties.get(key));
        assert_eq!(None, properties.get(key2));
    }
}
