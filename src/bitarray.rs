#[derive(Default)]
pub struct BitArray {
    data: Vec<u8>,
}

impl BitArray {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
    pub fn with_capacity(length: usize) -> Self {
        Self {
            data: Vec::with_capacity((length + 7) / 8),
        }
    }
    pub fn has(&self, index: usize) -> bool {
        if index / 8 >= self.data.len() {
            return false;
        }
        self.data[index / 8] & (1 << (index & 7)) != 0
    }
    pub fn add(&mut self, index: usize) {
        while index / 8 >= self.data.len() {
            self.data.push(0);
        }
        self.data[index / 8] |= 1 << (index & 7)
    }
    pub fn remove(&mut self, index: usize) {
        if index / 8 >= self.data.len() {
            return;
        }
        self.data[index / 8] &= !(1 << (index & 7))
    }
    
    pub fn clear(&mut self) {
        self.data.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primes() {
        let mut bit_array = BitArray::new();
        bit_array.add(2);
        bit_array.add(3);
        bit_array.add(5);
        bit_array.add(7);
        assert!(!bit_array.has(4));
        assert!(bit_array.has(5));
        bit_array.remove(5);
        assert!(!bit_array.has(5));
    }
}
