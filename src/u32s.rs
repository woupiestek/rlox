use crate::bitarray::BitArray;

pub struct U32s {
    data: Vec<u32>,
}

// a free list is implemented as an internal linked list
// the last position is always used as a pointer to the second to last free position
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

    // omit the 24 bytes of the struct
    pub fn byte_count(&self) -> usize {
        self.data.capacity() * 4
    }
}
