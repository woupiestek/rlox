use std::{mem, u32};

use crate::{
    handles::Handles,
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

pub struct Strings {
    handle_set: Box<[StringHandle]>,
    keys: Handles,
    mask: usize,
    // could be one big string and a vec of offsets...
    // even one Box<str>?
    // Box<[u8]> would be a better choice, simply because
    // it makes the intent to mutate clearer
    strings: Vec<Option<Box<str>>>,
    string_byte_count: usize,
}

impl Strings {
    const OFFSET: u32 = 16;
    pub fn new() -> Self {
        Self {
            handle_set: vec![StringHandle::EMPTY; 8].into_boxed_slice(),
            keys: Handles::new(),
            mask: 7, // self.handle_set.len() - 1
            strings: Vec::new(),
            string_byte_count: 0,
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
        assert!(self.strings.len() * 4 < self.handle_set.len() * 3);
        let mut index = self.hash(string);
        loop {
            match self.handle_set[index] {
                StringHandle::EMPTY => return (false, index),
                handle => {
                    if self.get(handle) == Some(string) {
                        return (true, index);
                    }
                }
            }
            index += 1;
            index &= self.mask;
        }
    }

    pub fn get(&self, handle: StringHandle) -> Option<&str> {
        self.strings[(handle.0 - Self::OFFSET) as usize].as_deref()
    }

    fn grow(&mut self) {
        let capacity = if self.handle_set.len() == 0 {
            8
        } else {
            self.handle_set.len() * 2
        };
        self.handle_set = vec![StringHandle::EMPTY; capacity].into_boxed_slice();
        self.mask = capacity - 1;
        for i in 0..self.strings.len() {
            if self.keys.is_marked(i as u32) {
                if let Some(str) = &self.strings[i] {
                    let (_, index) = self.find(str);
                    self.handle_set[index] = Handle(i as u32 + Self::OFFSET)
                }
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
        while self.strings.len() <= key as usize {
            self.strings.push(None);
        }
        self.strings[key as usize] = Some(Box::from(string));
        self.string_byte_count += string.len();
        let handle = Handle(key as u32 + Self::OFFSET);
        self.handle_set[index] = handle;
        handle
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
}

impl Pool<STRING> for Strings {
    fn byte_count(&self) -> usize {
        mem::size_of::<Strings>()
            + self.handle_set.len() * 4
            + self.strings.capacity() * mem::size_of::<Option<Box<str>>>()
            + self.keys.byte_count()
    }

    fn mark(&mut self, collector: &mut Collector) -> bool {
        if collector.handles[STRING].is_empty() {
            return true;
        }
        while let Some(key) = collector.handles[STRING].pop() {
            if key < Self::OFFSET {
                continue;
            }
            self.keys.mark(key - Self::OFFSET);
        }
        false
    }

    fn trace(&mut self, _handle: Handle<STRING>, _collector: &mut Collector) {}

    fn reset(&mut self) {
        self.keys.clear();
    }

    fn sweep(&mut self) {
        for i in 0..self.strings.len() {
            if !self.keys.is_marked(i as u32) {
                if let Some(str) = &self.strings[i] {
                    self.string_byte_count -= str.len();
                    self.strings[i] = None;
                }
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
        assert_eq!(Some("str"), strings.get(key));

        let key1 = strings.put("one");
        let key2 = strings.put("two");
        assert_eq!(key2, strings.put("two"));
        assert_eq!(Some("one"), strings.get(key1));
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
            assert_eq!(Some(values[i].as_str()), strings.get(handles[i]));
        }
    }
}
