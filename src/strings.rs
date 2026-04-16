use std::{mem, u32};

use crate::{
    handles::HandleSet,
    heap::{Collector, Handle, Pool, STRING},
};

pub type StringHandle = Handle<STRING>;

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
    keys: HandleSet,
    buffer: Buffer,
    offsets: Vec<u32>,
}

impl Strings {
    const OFFSET: u32 = 16;
    pub fn new() -> Self {
        Self {
            keys: HandleSet::new(),
            buffer: Buffer::with_capacity(0),
            offsets: Vec::new(),
        }
    }

    pub fn get(&self, handle: StringHandle) -> &str {
        self.buffer.get((handle.0 - Self::OFFSET) as usize)
    }

    pub fn put(&mut self, string: &str) -> StringHandle {
        let key = self.keys.next() as usize;
        while self.offsets.len() <= key as usize {
            self.offsets.push(u32::MAX);
        }
        self.offsets[key] = self.buffer.add(string) as u32;
        let handle = Handle(key as u32 + Self::OFFSET);
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
        mem::size_of::<Strings>() + self.buffer.byte_count() + self.keys.byte_count()
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
