use crate::bitarray::BitArray;

pub struct U32s {
    data: Vec<u32>,
}

// a free list is implemented as an internal linked list
// the last position is always used as a pointer to the seocnd to last free position
impl U32s {
    pub fn new() -> Self {
        Self { data: vec![0] }
    }

    pub fn store(&mut self, value: u32) -> u32 {
        let count = self.count();
        let free = self.data[count];
        if free as usize == count {
            self.data.push(free + 1);
        } else {
            self.data[count] = self.data[free as usize];
        }
        self.data[free as usize] = value;
        free
    }

    pub fn get(&self, index: u32) -> u32 {
        self.data[index as usize]
    }

    pub fn count(&self) -> usize {
        self.data.len() - 1
    }

    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    pub fn sweep(&mut self, marks: &BitArray) {
        let count = self.count();
        let mut free = count;
        for i in 0..count {
            if !marks.has(i) {
                self.data[i] = free as u32;
                free = i;
            }
        }
        self.data[count] = free as u32;
    }

    // ouch...
    pub fn free_indices(&self) -> FreeIterator {
        FreeIterator {
            u32s: self,
            index: self.data[self.count()] as usize,
        }
    }
}

pub struct FreeIterator<'m> {
    u32s: &'m U32s,
    index: usize,
}

// note type members...
impl<'m> Iterator for FreeIterator<'m> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.u32s.count() {
            return None;
        }
        let result = Some(self.index);
        self.index = self.u32s.data[self.index] as usize;
        result
    }
}
