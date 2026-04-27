use std::{
    mem,
    ops::{Index, IndexMut, Range},
};

use crate::symbols::SymbolHandle;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeySet {
    ptr: u32,
    size: u16,
    len: u16,
}

impl KeySet {
    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn ptr(&self) -> usize {
        3 * self.ptr as usize
    }

    pub fn range(&self) -> Range<usize> {
        self.ptr()..self.ptr() + self.len()
    }
}

pub struct KeySets {
    keys: Vec<SymbolHandle>,
    indices: Vec<u16>,
    free: Vec<Vec<u32>>,
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

    fn hash(&self, set: KeySet, key: SymbolHandle) -> (bool, usize) {
        if set.size == 0 {
            return (false, usize::MAX);
        }
        let mask = 4 * set.size - 1;
        let mut hash = (key.0 as u16).reverse_bits() >> mask.leading_zeros();
        let offset = set.ptr as usize;
        loop {
            let i = 4 * offset + hash as usize;
            if self.indices[i] == Self::UNDEFINED {
                return (false, i);
            }
            if self.keys[3 * offset + self.indices[i] as usize] == key {
                return (true, i);
            }
            hash = (hash + 1) & mask
        }
    }

    pub fn find(&self, set: KeySet, key: SymbolHandle) -> Option<usize> {
        let (matched, i) = self.hash(set, key);
        if matched {
            Some(self.indices[i] as usize)
        } else {
            None
        }
    }

    fn _alloc(&mut self, size: u32) -> u32 {
        let order = size.ilog2() as usize;
        if order < self.free.len() {
            let offset = if let Some(offset) = self.free[order].pop() {
                offset
            } else {
                let offset = self._alloc(2 * size);
                self.free[order].push(offset | size);
                offset
            };
            for i in 0..4 * size as usize {
                self.indices[4 * offset as usize + i] = Self::UNDEFINED;
            }
            return offset;
        }

        let offset = if self.keys.len() == 0 {
            // zero step
            self.free = vec![vec![]; order];
            self.free.push(vec![size]);
            0
        } else {
            // inductive step
            for p in self.free.len()..order {
                self.free.push(vec![(1 << p) as u32]);
            }
            self.free.push(Vec::new());
            size
        };
        self.keys.resize(6 * size as usize, SymbolHandle::EMPTY);
        self.indices.resize(8 * size as usize, Self::UNDEFINED);
        offset
    }

    fn alloc(&mut self, size: u16) -> KeySet {
        return KeySet {
            len: 0,
            ptr: self._alloc(size as u32),
            size,
        };
    }

    fn _free(&mut self, offset: u32, size: u16) {
        let order = size.ilog2() as usize;
        // buddy search
        let buddy = offset ^ size as u32;
        for i in 0..self.free[order].len() {
            if self.free[order][i] == buddy {
                self.free[order].swap_remove(i);
                self._free(offset & buddy, size * 2);
                return;
            }
        }
        self.free[order].push(offset);
    }

    pub fn free(&mut self, set: KeySet) {
        if set == KeySet::default() {
            return;
        }
        self._free(set.ptr, set.size);
    }

    // assume any growing is done
    fn push(&mut self, set: &mut KeySet, key: SymbolHandle, index: usize) {
        self.keys[set.ptr() + set.len()] = key;
        self.indices[index] = set.len;
        set.len += 1
    }

    fn grow_set(&mut self, set: KeySet) -> KeySet {
        let mut new_set = self.alloc((set.size + 1).next_power_of_two());
        for i in 0..set.len() {
            let key = self.keys[set.ptr() + i];
            let (_, index) = self.hash(new_set, key);
            self.push(&mut new_set, key, index);
        }
        self.free(set);
        new_set
    }

    pub fn add(&mut self, mut set: KeySet, key: SymbolHandle) -> KeySet {
        let (matched, index) = self.hash(set, key);
        if matched {
            return set;
        }
        if set.size * 3 > set.len {
            self.push(&mut set, key, index);
            return set;
        }
        let mut new_set = self.grow_set(set);
        let (_, index) = self.hash(new_set, key);
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

impl Index<usize> for KeySets {
    type Output = SymbolHandle;

    fn index(&self, index: usize) -> &Self::Output {
        &self.keys[index]
    }
}

impl Index<Range<usize>> for KeySets {
    type Output = [SymbolHandle];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.keys[index]
    }
}

impl IndexMut<usize> for KeySets {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.keys[index]
    }
}
