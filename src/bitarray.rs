pub struct BitArray {
    length: usize,
    data: Box<[u8]>,
}

impl BitArray {
    pub fn new(length: usize) -> Self {
        Self {
            length,
            data: Box::from(vec![0; (length + 7) / 8]),
        }
    }
    pub fn get(&self, index: usize) -> bool {
        assert!(index < self.length, "Index out of bounds");
        self.data[index / 8] & (1 << (index & 7)) != 0
    }
    pub fn add(&mut self, index: usize) {
        assert!(index < self.length, "Index out of bounds");
        self.data[index / 8] |= 1 << (index & 7)
    }
    pub fn remove(&mut self, index: usize) {
        assert!(index < self.length, "Index out of bounds");
        self.data[index / 8] &= !(1 << (index & 7))
    }
}
