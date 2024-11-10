use crate::{
    bitarray::BitArray,
    classes::ClassHandle,
    heap::{Collector, Handle, Heap, Pool, INSTANCE},
    strings::StringHandle,
    u32s::U32s,
    values::Value,
};

struct Properties {
    count: usize,
    keys: Box<[StringHandle]>,
    values: Box<[Value]>,
}

impl Properties {
    fn byte_count(&self) -> usize {
        40 + self.keys.len() * 12
    }

    fn with_capacity(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "what were you thinking?");
        Self {
            count: 0,
            keys: vec![StringHandle::EMPTY; capacity].into_boxed_slice(),
            values: vec![Value::NIL; capacity].into_boxed_slice(),
        }
    }

    fn capacity(&self) -> usize {
        self.keys.len()
    }

    fn find(&self, key: StringHandle) -> (bool, usize) {
        assert!(4 * self.count <= 3 * self.capacity());
        let mask = self.capacity() - 1;
        let mut index = key.0 as usize & mask;
        loop {
            let string_handle = self.keys[index as usize];
            if string_handle == StringHandle::EMPTY {
                return (false, index);
            }
            if string_handle == key {
                return (true, index);
            }
            index = (index + 1) & mask;
        }
    }

    fn get(&self, key: StringHandle) -> Option<Value> {
        let (found, index) = self.find(key);
        if found {
            Some(self.values[index as usize])
        } else {
            None
        }
    }

    fn put(&mut self, key: StringHandle, value: Value) {
        let (found, index) = self.find(key);
        self.values[index as usize] = value;
        if found {
            return;
        }
        self.keys[index as usize] = key;
        self.count += 1;
    }
}

pub type InstanceHandle = Handle<INSTANCE>;

pub struct Instances {
    byte_count: usize,
    classes: U32s,
    properties: Vec<Properties>,
}

impl Instances {
    pub fn new() -> Self {
        Self {
            byte_count: 56,
            classes: U32s::new(),
            properties: Vec::new(),
        }
    }

    pub fn new_instance(&mut self, class: ClassHandle) -> InstanceHandle {
        let index = self.classes.store(class.0);
        if index < self.properties.len() as u32 {
            self.byte_count -= self.properties[index as usize].byte_count();
            self.properties[index as usize] = Properties::with_capacity(8);
            self.byte_count += self.properties[index as usize].byte_count();
        } else {
            while index >= self.properties.len() as u32 {
                let properties = Properties::with_capacity(8);
                self.byte_count += properties.byte_count();
                self.properties.push(properties);
            }
        }
        InstanceHandle::from(index)
    }

    pub fn get_class<'s>(&self, ih: InstanceHandle) -> ClassHandle {
        ClassHandle::from(self.classes.get(ih.0))
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

    fn grow(&mut self, ih: InstanceHandle) {
        let old_properties = &self.properties[ih.index()];
        let mut new_properties = Properties::with_capacity(old_properties.capacity() * 2);
        for index in 0..self.properties[ih.index()].capacity() {
            let key = self.properties[ih.index()].keys[index as usize];
            if key == StringHandle::EMPTY {
                continue;
            }
            new_properties.put(key, self.properties[ih.index()].values[index as usize]);
        }
        self.byte_count += new_properties.byte_count() - old_properties.byte_count();
        self.properties[ih.index()] = new_properties;
    }

    pub fn set_property(&mut self, ih: InstanceHandle, key: StringHandle, value: Value) {
        if 4 * (self.properties[ih.index()].count + 1) > 3 * self.properties[ih.index()].capacity()
        {
            self.grow(ih);
        }
        self.properties[ih.index()].put(key, value)
    }
}

impl Pool<INSTANCE> for Instances {
    fn byte_count(&self) -> usize {
        self.byte_count + self.classes.byte_count()
    }
    fn trace(&self, handle: Handle<INSTANCE>, collector: &mut Collector) {
        collector
            .keys
            .push(StringHandle(self.classes.get(handle.0)));
        for index in 0..self.properties[handle.index()].capacity() {
            let key = self.properties[handle.index()].keys[index as usize];
            if key == StringHandle::EMPTY {
                continue;
            }
            collector.keys.push(key);
            self.properties[handle.index()].values[index as usize].trace(collector);
        }
    }
    fn sweep(&mut self, marks: &BitArray) {
        self.classes.sweep(marks);
    }
    fn count(&self) -> usize {
        self.classes.count()
    }
}
