use std::{mem, ops::Range, u32};

use crate::{
    handles::{Column, HandleSet},
    heap::{Collector, Handle, Pool, STRING},
};

pub type StringHandle = Handle<STRING>;

struct Buffer {
    string: String,
}

impl Buffer {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            string: String::with_capacity(capacity),
        }
    }

    fn get(&self, range: Range<usize>) -> &str {
        &self.string[range]
    }

    fn add(&mut self, str: &str) -> Range<usize> {
        let from = self.string.len();
        self.string.push_str(str);
        from..self.string.len()
    }

    fn byte_count(&self) -> usize {
        mem::size_of::<Self>() + self.string.capacity()
    }
}

pub struct Strings {
    keys: HandleSet,
    buffer: Buffer,
    ranges: Column<Range<usize>>,
}

impl Strings {
    pub fn new() -> Self {
        Self {
            keys: HandleSet::new(),
            buffer: Buffer::with_capacity(0),
            ranges: Column::new(),
        }
    }

    pub fn get(&self, handle: StringHandle) -> &str {
        self.buffer.get(self.ranges.get(handle.0))
    }

    pub fn put(&mut self, string: &str) -> StringHandle {
        let key = self.keys.next();
        self.ranges.set(key, self.buffer.add(string));
        let handle = Handle(key);
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
        self.keys.mark(key)
    }

    fn trace_all(&mut self, _marked: &Vec<u32>, _collector: &mut Collector) {}

    fn reset(&mut self) {
        self.keys.clear();
    }

    fn sweep(&mut self) {
        if self.keys.count() * 4 > self.keys.len() * 3 {
            // don't compactify yet
            return;
        }
        let capacity = self.buffer.string.capacity();
        let buffer = mem::replace(&mut self.buffer, Buffer::with_capacity(capacity));
        for i in 0..self.keys.len() as u32 {
            if self.keys.is_marked(i as u32) {
                self.ranges
                    .set(i, self.buffer.add(buffer.get(self.ranges.get(i))));
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn string_inequality() {
        // this is for literals strings, so the uniqueness guarantees for symbols do not hold.
        let mut strings = Strings::new();
        let key = strings.put("str");
        assert_ne!(key, strings.put("str"));
        assert_eq!("str", strings.get(key));

        let key1 = strings.put("one");
        let key2 = strings.put("two");
        assert_ne!(key2, strings.put("two"));
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
