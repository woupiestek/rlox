use std::{mem, ops::Range, u32};

use crate::{
    handles::{Column, HandleSet},
    heap::{Collector, Handle, Pool, SYMBOL},
};

pub type SymbolHandle = Handle<SYMBOL>;

impl SymbolHandle {
    pub const EMPTY: Self = Self(u32::MAX);
    pub fn is_valid(&self) -> bool {
        self != &SymbolHandle::EMPTY
    }
}

struct Buffer {
    data: String,
}

impl Buffer {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            data: String::with_capacity(capacity),
        }
    }

    fn get(&self, range: Range<u32>) -> &str {
        &self.data[range.start as usize..range.end as usize]
    }

    fn add(&mut self, str: &str) -> Range<u32> {
        let from = self.data.len() as u32;
        self.data.push_str(str);
        from..self.data.len() as u32
    }

    fn byte_count(&self) -> usize {
        mem::size_of::<Self>() + self.data.capacity()
    }
}

pub struct Symbols {
    indices: Box<[SymbolHandle]>,
    keys: HandleSet,
    mask: usize,
    buffer: Buffer,
    ranges: Column<Range<u32>>,
}

impl Symbols {
    pub fn new() -> Self {
        Self {
            indices: vec![SymbolHandle::EMPTY; 8].into_boxed_slice(),
            keys: HandleSet::new(),
            mask: 7, // self.handle_set.len() - 1
            buffer: Buffer::with_capacity(0),
            ranges: Column::new(),
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

    fn find(&self, symbol: &str) -> (bool, usize) {
        let mut hash = self.hash(symbol);
        loop {
            if self.indices[hash] == SymbolHandle::EMPTY {
                return (false, hash);
            }
            if self.get(self.indices[hash]) == symbol {
                return (true, hash);
            }
            hash += 1;
            hash &= self.mask;
        }
    }

    pub fn get(&self, handle: SymbolHandle) -> &str {
        self.buffer.get(self.ranges.get(handle.0))
    }

    fn grow(&mut self) {
        let capacity = if self.indices.len() == 0 {
            8
        } else {
            self.indices.len() * 2
        };
        self.indices = vec![SymbolHandle::EMPTY; capacity].into_boxed_slice();
        self.mask = capacity - 1;
        for i in 0..self.keys.len() as u32 {
            if self.keys.is_marked(i) {
                let str = self.buffer.get(self.ranges.get(i));
                let (_, index) = self.find(str);
                self.indices[index] = Handle(i);
            }
        }
    }

    pub fn put(&mut self, symbol: &str) -> SymbolHandle {
        if (self.keys.count() + 1) * 4 > self.indices.len() * 3 {
            self.grow();
        }

        let (found, index) = self.find(symbol);
        if found {
            return self.indices[index];
        }

        let key = self.keys.next();
        self.ranges.set(key, self.buffer.add(symbol));
        let handle = Handle(key);
        self.indices[index] = handle;
        handle
    }
}

impl Pool<SYMBOL> for Symbols {
    fn byte_count(&self) -> usize {
        mem::size_of::<Symbols>()
            + self.indices.len() * 4
            + self.ranges.byte_count()
            + self.buffer.byte_count()
            + self.keys.byte_count()
    }

    fn mark(&mut self, key: u32) -> bool {
        if key < u32::MAX {
            self.keys.mark(key)
        } else {
            false
        }
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
        let capacity = self.buffer.data.capacity();
        let buffer = mem::replace(&mut self.buffer, Buffer::with_capacity(capacity));
        for i in 0..self.ranges.values.len() as u32 {
            if self.keys.is_marked(i) {
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
    pub fn symbol_equality() {
        let mut symbols = Symbols::new();
        let key = symbols.put("str");
        assert_eq!(key, symbols.put("str"));
        assert_eq!("str", symbols.get(key));

        let key1 = symbols.put("one");
        let key2 = symbols.put("two");
        assert_eq!(key2, symbols.put("two"));
        assert_eq!("one", symbols.get(key1));
        assert_ne!(key1, key2);
    }

    #[test]
    pub fn growth() {
        let mut symbols = Symbols::new();
        let mut handles: Vec<SymbolHandle> = Vec::new();
        let mut values: Vec<String> = Vec::new();
        for i in 0..12 {
            let value = "str".to_owned() + &i.to_string();
            handles.push(symbols.put(&value));
            values.push(value.clone());
        }
        for i in 0..12 {
            assert_eq!(values[i].as_str(), symbols.get(handles[i]));
        }
    }
}
