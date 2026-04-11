use std::{mem, u32};

use crate::{
    handles::HandleSet,
    heap::{Collector, Handle, Pool, STRING},
};

pub type StringHandle = Handle<STRING>;

impl StringHandle {
    pub const EMPTY: Self = Self(0);
    pub const TOMBSTONE: Self = Self(1);
    pub fn is_valid(&self) -> bool {
        self != &StringHandle::EMPTY && self != &StringHandle::TOMBSTONE
    }
}

struct Buffer {
    string: String,
    tos: Vec<u32>,
}

impl Buffer {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            string: String::with_capacity(capacity),
            tos: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.tos.len()
    }

    fn get(&self, index: usize) -> &str {
        let from = if index == 0 { 0 } else { self.tos[index - 1] } as usize;
        let to = self.tos[index] as usize;
        &self.string[from..to]
    }

    fn add(&mut self, str: &str) -> usize {
        self.string.push_str(str);
        let index = self.tos.len();
        self.tos.push(self.string.len() as u32);
        index
    }

    fn byte_count(&self) -> usize {
        mem::size_of::<Self>() + self.string.capacity() + self.tos.capacity() * 4
    }
}

pub struct Strings {
    handle_set: Box<[StringHandle]>,
    keys: HandleSet,
    mask: usize,
    buffer: Buffer,
    offsets: Vec<u32>,
}

impl Strings {
    const OFFSET: u32 = 16;
    pub fn new() -> Self {
        Self {
            handle_set: vec![StringHandle::EMPTY; 8].into_boxed_slice(),
            keys: HandleSet::new(),
            mask: 7, // self.handle_set.len() - 1
            buffer: Buffer::with_capacity(0),
            offsets: Vec::new(),
        }
    }

    fn hash(&self, str: &str) -> usize {
        let mut hash = 2166136261u32;
        for &byte in str.as_bytes() {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(16777619u32);
        }
        hash as usize & self.mask
    }

    fn find(&self, string: &str) -> (bool, usize) {
        assert!(self.buffer.len() * 4 < self.handle_set.len() * 3);
        let mut index = self.hash(string);
        loop {
            match self.handle_set[index] {
                StringHandle::EMPTY => return (false, index),
                handle => {
                    if self.get(handle) == string {
                        return (true, index);
                    }
                }
            }
            index += 1;
            index &= self.mask;
        }
    }

    pub fn get(&self, handle: StringHandle) -> &str {
        self.buffer.get((handle.0 - Self::OFFSET) as usize)
    }

    fn grow(&mut self) {
        let capacity = if self.handle_set.len() == 0 {
            8
        } else {
            self.handle_set.len() * 2
        };
        self.handle_set = vec![StringHandle::EMPTY; capacity].into_boxed_slice();
        self.mask = capacity - 1;
        for i in 0..self.buffer.len() {
            if self.keys.is_marked(i as u32) {
                let str = self.buffer.get(i);
                let (_, index) = self.find(str);
                self.handle_set[index] = Handle(i as u32 + Self::OFFSET)
            }
        }
    }

    pub fn put(&mut self, string: &str) -> StringHandle {
        if (self.keys.count() + 1) * 4 > self.handle_set.len() * 3 {
            self.grow();
        }

        let (found, index) = self.find(string);
        if found {
            return self.handle_set[index];
        }

        let key = self.keys.next() as usize;
        while self.offsets.len() <= key as usize {
            self.offsets.push(u32::MAX);
        }
        self.offsets[key] = self.buffer.add(string) as u32;
        let handle = Handle(key as u32 + Self::OFFSET);
        self.handle_set[index] = handle;
        handle
    }

    pub fn concat(&mut self, a: StringHandle, b: StringHandle) -> StringHandle {
        let mut c = String::new();
        c.push_str(self.get(a));
        c.push_str(self.get(b));
        self.put(&c)
    }
}

impl Pool<STRING> for Strings {
    fn byte_count(&self) -> usize {
        mem::size_of::<Strings>()
            + self.handle_set.len() * 4
            + self.buffer.byte_count()
            + self.keys.byte_count()
    }

    fn mark(&mut self, key: u32) -> bool {
        // let's just be honest
        if key >= Self::OFFSET {
            self.keys.mark(key - Self::OFFSET)
        } else {
            false
        }
    }

    fn trace_all(&mut self, _marked: &Vec<u32>, _collector: &mut Collector) {}

    fn reset(&mut self) {
        self.keys.clear();
    }

    fn sweep(&mut self) {
        if self.keys.count() * 4 > self.buffer.len() * 3 {
            // don't compactify yet
            return;
        }
        let capacity = self.buffer.string.capacity();
        let buffer = mem::replace(&mut self.buffer, Buffer::with_capacity(capacity));
        for i in 0..self.offsets.len() {
            if self.keys.is_marked(i as u32) {
                self.offsets[i] = self.buffer.add(buffer.get(self.offsets[i] as usize)) as u32;
            } else {
                self.offsets[i] = u32::MAX;
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn string_equality() {
        let mut strings = Strings::new();
        let key = strings.put("str");
        assert_eq!(key, strings.put("str"));
        assert_eq!("str", strings.get(key));

        let key1 = strings.put("one");
        let key2 = strings.put("two");
        assert_eq!(key2, strings.put("two"));
        assert_eq!("one", strings.get(key1));
        assert_ne!(key1, key2);
    }

    #[test]
    pub fn growth() {
        let mut strings = Strings::new();
        let mut handles: Vec<StringHandle> = Vec::new();
        let mut values: Vec<String> = Vec::new();
        for i in 0..12 {
            let value = "str".to_owned() + &i.to_string();
            handles.push(strings.put(&value));
            values.push(value.clone());
        }
        for i in 0..12 {
            assert_eq!(values[i].as_str(), strings.get(handles[i]));
        }
    }
}
