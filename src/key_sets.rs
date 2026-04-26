use std::mem;

use crate::symbols::SymbolHandle;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeySet {
    offset: u32,
    size: u16,
    len: u16,
}

impl KeySet {
    pub fn key_len(&self) -> usize {
        self.len as usize
    }

    fn key_offset(&self) -> usize {
        6 * self.offset as usize
    }

    fn index_offset(&self) -> usize {
        8 * self.offset as usize
    }
}

pub struct KeySets {
    keys: Vec<SymbolHandle>,
    indices: Vec<u16>,
    free: Vec<KeySet>,
}

impl KeySets {
    const UNDEFINED: u16 = u16::MAX;

    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            indices: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn get_ref(&self, set: KeySet) -> &[SymbolHandle] {
        &self.keys[set.key_offset()..set.key_offset() + set.key_len()]
    }

    pub fn get(&self, set: KeySet, index: usize) -> SymbolHandle {
        self.keys[set.key_offset() + index]
    }

    fn hash(&self, set: &KeySet, key: SymbolHandle) -> (bool, usize) {
        if set.size == 0 {
            return (false, usize::MAX);
        }
        let mask = 8 * set.size - 1;
        let mut hash = (key.0 as u16).reverse_bits() >> mask.leading_zeros();
        let offset = set.offset as usize;
        loop {
            let i = 8 * offset + hash as usize;
            if self.indices[i] == Self::UNDEFINED {
                return (false, i);
            }
            if self.keys[6 * offset + self.indices[i] as usize] == key {
                return (true, i);
            }
            hash = (hash + 1) & mask
        }
    }

    pub fn find(&self, set: KeySet, key: SymbolHandle) -> Option<usize> {
        let (matched, i) = self.hash(&set, key);
        if matched {
            Some(self.indices[i] as usize)
        } else {
            None
        }
    }

    fn alloc(&mut self, new_size: u16) -> KeySet {
        for i in (0..self.free.len()).rev() {
            if self.free[i].size != new_size {
                continue;
            }
            let mut set = self.free[i];
            let last = self.free.pop().unwrap();
            if i < self.free.len() {
                self.free[i] = last;
            }
            set.len = 0;
            let from = set.index_offset();
            let to = from + 8 * new_size as usize;
            for i in from..to as usize {
                self.indices[i] = Self::UNDEFINED;
            }
            return set;
        }
        let set = KeySet {
            len: 0,
            size: new_size,
            offset: (self.indices.len() >> 3) as u32,
        };
        let min_keys_len = set.key_offset() + 6 * set.size as usize;
        self.keys.resize(min_keys_len, SymbolHandle::EMPTY);
        let min_indices_len = 8 * (set.offset as usize + set.size as usize);
        // no next power of two because incides len is used a few lines before.
        self.indices.resize(min_indices_len, Self::UNDEFINED);
        set
    }

    pub fn free(&mut self, set: KeySet) {
        if set == KeySet::default() {
            return;
        }
        self.free.push(set);
    }

    // assume any growing is done
    fn push(&mut self, set: &mut KeySet, key: SymbolHandle, index: usize) {
        self.keys[set.key_offset() + set.key_len()] = key;
        self.indices[index] = set.len;
        set.len += 1
    }

    fn grow(&mut self, set: KeySet) -> KeySet {
        let mut new_set = self.alloc((set.size + 1).next_power_of_two());
        for i in 0..set.key_len() {
            let key = self.keys[set.key_offset() + i];
            let (_, index) = self.hash(&new_set, key);
            self.push(&mut new_set, key, index);
        }
        self.free(set);
        new_set
    }

    pub fn add(&mut self, mut set: KeySet, key: SymbolHandle) -> KeySet {
        let (matched, index) = self.hash(&set, key);
        if matched {
            return set;
        }
        if set.size * 6 > set.len {
            self.push(&mut set, key, index);
            return set;
        }
        let mut new_set = self.grow(set);
        let (_, index) = self.hash(&new_set, key);
        self.push(&mut new_set, key, index);
        new_set
    }

    pub fn byte_count(&self) -> usize {
        mem::size_of::<Self>()
            + self.keys.capacity() * mem::size_of::<SymbolHandle>()
            + self.indices.capacity() * mem::size_of::<u16>()
            + self.free.capacity() * mem::size_of::<KeySet>()
    }
}
