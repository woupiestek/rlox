#[derive(Default)]
pub struct BitArray {
    data: Vec<u8>,
}

impl BitArray {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn has(&self, index: usize) -> bool {
        let i = index / 8;
        if i >= self.data.len() {
            return false;
        }
        self.data[i] & (1 << (index & 7)) != 0
    }

    // return true if the set is changed
    fn save(&mut self, i: usize, d: u8) -> bool {
        if d == self.data[i] {
            false
        } else {
            self.data[i] = d;
            true
        }
    }

    pub fn add(&mut self, index: usize) -> bool {
        let i = index / 8;
        while i >= self.data.len() {
            self.data.push(0);
        }
        self.save(i, self.data[i] | 1 << (index & 7))
    }

    pub fn remove(&mut self, index: usize) -> bool {
        let i = index / 8;
        if i >= self.data.len() {
            return false;
        }
        self.save(i, self.data[i] & !(1 << (index & 7)))
    }

    pub fn truncate(&mut self, index: usize) -> bool {
        let i = index / 8;
        if i >= self.data.len() {
            return false;
        }
        self.data.truncate(i + 1);
        self.save(i, self.data[i] & ((1 << (index & 7)) - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primes() {
        // small test...
        let mut bit_array = BitArray::new();
        assert!(bit_array.add(2));
        assert!(bit_array.add(3));
        assert!(bit_array.add(5));
        assert!(bit_array.add(7));
        assert!(!bit_array.has(4));
        assert!(bit_array.has(5));
        assert!(bit_array.remove(5));
        assert!(!bit_array.has(5));
    }
}
