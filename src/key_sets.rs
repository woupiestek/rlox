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

    fn _alloc(&mut self, size: u16) -> Option<u32> {
        let order = size.ilog2() as usize;
        if order >= self.free.len() {
            return None;
        }
        if let Some(offset) = self.free[order].pop() {
            return Some(offset);
        }
        if let Some(offset) = self._alloc(size * 2) {
            self.free[order].push(offset | size as u32);
            return Some(offset);
        }
        None
    }

    fn _grow(&mut self, size: u16) -> u32 {
        let order = size.ilog2() as usize;
        if self.free.len() < order {
            self.free.resize_with(order, Vec::new);
        }
        let mut len = (self.indices.len() / 8) as u32;
        for i in 0..order {
            if len.is_multiple_of(2 << i) {
                continue;
            }
            self.free[i].push(len);
            len = len.next_multiple_of(2 << i);
        }
        len
    }

    fn alloc(&mut self, size: u16) -> KeySet {
        if let Some(offset) = self._alloc(size) {
            let from = 8 * offset as usize;
            let to = from + 8 * size as usize;
            for i in from..to as usize {
                self.indices[i] = Self::UNDEFINED;
            }
            return KeySet {
                offset,
                size,
                len: 0,
            };
        }
        let offset = self._grow(size);
        let len = offset as usize + size as usize;
        self.keys.resize(6 * len, SymbolHandle::EMPTY);
        self.indices.resize(8 * len, Self::UNDEFINED);
        KeySet {
            len: 0,
            size,
            offset,
        }
    }

    fn _free(&mut self, offset: u32, size: u16) {
        let order = size.ilog2() as usize;
        // buddy search
        let buddy = offset ^ size as u32;
        if order < self.free.len() {
            for i in 0..self.free[order].len() {
                if self.free[order][i] == buddy {
                    self.free[order].swap_remove(i);
                    self._free(offset & buddy, size * 2);
                    return;
                }
            }
            if order >= self.free.len() {
                self.free.resize_with(order + 1, Vec::new);
            }
            self.free[order].push(offset);
        }
    }

    pub fn free(&mut self, set: KeySet) {
        if set == KeySet::default() {
            return;
        }
        self._free(set.offset, set.size);
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

// struct Buddies {
//     // free lists at all levels
//     free: Vec<Vec<usize>>,
// }

// impl Buddies {
//     pub fn new() -> Self {
//         Self {
//             free: vec![vec![0]],
//         }
//     }

//     pub fn free(&mut self, id: usize, co_order: usize) {
//         assert!(id < (1 << co_order));

//         if id > 0 && co_order > 0 && co_order < self.free.len() {
//             let buddy = id ^ (1 << (co_order - 1));
//             // search buddy in the free list,
//             // actually putting block together to keep the free lists short
//             for i in 0..self.free[co_order].len() {
//                 if self.free[co_order][i] == buddy {
//                     self.free[co_order].swap_remove(i);
//                     self.free(buddy, co_order - 1);
//                     return;
//                 }
//             }
//         }

//         self.free[co_order].push(id);
//     }

//     pub fn alloc(&mut self, co_order: usize) -> Option<usize> {
//         if co_order < self.free.len() {
//             if let Some(id) = self.free[co_order].pop() {
//                 return Some(id);
//             }
//         }

//         if co_order == 0 {
//             return None;
//         }

//         if let Some(id) = self.alloc(co_order - 1) {
//             if co_order >= self.free.len() {
//                 self.free.resize_with(co_order + 1, Vec::new)
//             }
//             self.free[co_order].push(id ^ (1 << (co_order - 1)));
//             return Some(id);
//         }

//         None
//     }
// }
