pub struct BitArray {
    length: usize,
    data: Box<[u8]>,
}

impl BitArray {
    pub fn new(length: usize) -> Self {
        Self {
            length,
            data: vec![0; (length + 7) / 8].into_boxed_slice(),
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primes() {
        let mut bit_array = BitArray::new(10);
        bit_array.add(2);
        bit_array.add(3);
        bit_array.add(5);
        bit_array.add(7);
        assert!(!bit_array.get(4));
        assert!(bit_array.get(5));
        bit_array.remove(5);
        assert!(!bit_array.get(5));
    }
}
