use crate::bitarray::BitArray;

pub struct U32s {
    count: usize,
    data: Vec<u32>,
}

impl U32s {
    pub fn new() -> Self {
        Self {
            count: 0,
            data: Vec::new(),
        }
    }

    pub fn store(&mut self, value: u32) -> u32 {
        let l = self.data.len();
        if l > self.count {
            let i = self.data.pop().unwrap();
            self.data[i as usize] = value;
            i
        } else {
            self.data.push(value);
            self.count += 1;
            l as u32
        }
    }

    pub fn get(&self, index: u32) -> u32 {
        self.data[index as usize]
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    pub fn sweep(&mut self, marks: &BitArray) {
        self.data.truncate(self.count as usize);
        for i in 0..self.count {
            if !marks.has(i as usize) {
                self.data.push(i as u32);
                self.data[i as usize] = 0;
            }
        }
    }

    pub fn free_indices(&self) -> &[u32] {
        &self.data[self.count as usize..]
    }
}
