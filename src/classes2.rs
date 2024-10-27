use crate::{
    bitarray::BitArray,
    closures::ClosureHandle,
    heap::{Collector, Handle, Pool, CLASS},
    strings::{StringHandle, Strings},
    u32s::U32s,
};

struct Method {
    count: usize,
    keys: Box<[StringHandle]>,
    closures: Box<[ClosureHandle]>,
}

impl Method {
    fn byte_count(&self) -> usize {
        40 + self.keys.len() * 8
    }

    fn with_capacity(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "what were you thinking?");
        Self {
            count: 0,
            keys: vec![StringHandle::EMPTY; capacity].into_boxed_slice(),
            closures: vec![ClosureHandle::from(0); capacity].into_boxed_slice(),
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

    fn get(&self, key: StringHandle) -> Option<ClosureHandle> {
        let (found, index) = self.find(key);
        if found {
            Some(self.closures[index as usize])
        } else {
            None
        }
    }

    fn put(&mut self, key: StringHandle, ch: ClosureHandle) {
        let (found, index) = self.find(key);
        self.closures[index as usize] = ch;
        if found {
            return;
        }
        self.keys[index as usize] = key;
        self.count += 1;
    }
}

pub type ClassHandle = Handle<CLASS>;

pub struct Classes {
    byte_count: usize,
    names: U32s,
    methods: Vec<Method>,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            byte_count: 56,
            names: U32s::new(),
            methods: Vec::new(),
        }
    }

    pub fn new_class(&mut self, name: StringHandle) -> ClassHandle {
        let i = self.names.store(name.0);
        let batch = i >> 8;
        while batch >= self.methods.len() as u32 {
            let batch = Method::with_capacity(8);
            self.byte_count += batch.byte_count();
            self.methods.push(batch);
        }
        ClassHandle::from(i)
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, strings: &'s Strings) -> &'s str {
        strings.get(StringHandle(self.names.get(ch.0))).unwrap()
    }

    pub fn to_string(&self, ch: ClassHandle, strings: &Strings) -> String {
        format!("<class {}>", self.get_name(ch, strings))
    }

    pub fn get_method(&self, ch: ClassHandle, name: StringHandle) -> Option<ClosureHandle> {
        self.methods[ch.index()].get(name)
    }

    fn grow(&mut self, ch: ClassHandle) {
        let old_batch = &self.methods[ch.index()];
        let mut new_batch = Method::with_capacity(old_batch.capacity() * 2);
        for index in 0..old_batch.capacity() {
            if old_batch.keys[index] == StringHandle::EMPTY {
                new_batch.put(old_batch.keys[index], old_batch.closures[index]);
            }
        }
        self.byte_count += new_batch.capacity() - old_batch.capacity();
        self.methods[ch.index()] = new_batch;
    }

    pub fn set_method(&mut self, ch: ClassHandle, name: StringHandle, method: ClosureHandle) {
        if 4 * (self.methods[ch.index()].count + 1) > 3 * self.methods[ch.index()].capacity() {
            self.grow(ch);
        }
        self.methods[ch.index()].put(name, method)
    }

    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        for i in 0..self.methods[super_class.index()].capacity() {
            if self.methods[super_class.index()].keys[i] == StringHandle::EMPTY {
                continue;
            }
            self.set_method(
                sub_class,
                self.methods[super_class.index()].keys[i],
                self.methods[super_class.index()].closures[i],
            );
        }
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        self.byte_count
    }
    fn trace(&self, handle: Handle<CLASS>, collector: &mut Collector) {
        collector.keys.push(StringHandle(self.names.get(handle.0)));
        for index in 0..self.methods[handle.index()].capacity() {
            if self.methods[handle.index()].keys[index] == StringHandle::EMPTY {
                continue;
            }
            collector
                .keys
                .push(self.methods[handle.index()].keys[index]);
            collector.push(self.methods[handle.index()].closures[index]);
        }
    }
    fn sweep(&mut self, marks: &BitArray) {
        self.names.sweep(marks);
        for i in self.names.free_indices() {
            self.byte_count -= self.methods[i].byte_count();
            self.methods[i] = Method::with_capacity(8);
            self.byte_count += self.methods[i].byte_count();
        }
    }
    fn count(&self) -> usize {
        self.names.count()
    }
}
