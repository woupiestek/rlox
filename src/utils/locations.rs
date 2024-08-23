
pub struct Locations {
    last: u32,
    diffs: Vec<u8>,
}

impl Locations {
    pub fn new(location: u32) -> Self {
        Self {
            last: location as u32,
            diffs: Vec::new(),
        }
    }
    pub fn add(&mut self, location: u32) {
        let mut diff = location - self.last;
        self.last = location;
        // how do we do this?
        loop {
            let value = (diff & 0x7F) as u8;
            diff >>= 7;
            if diff == 0 {
                // leading 1 indicates the start
                self.diffs.push(value | 0x80);
                return;
            }
            // leading 0 indicates continuation
            self.diffs.push(value);
        }
    }
    // we don't know how far from the start, or do we?
    // does not matter, compute it from the end!
    pub fn get(&self, index: usize) -> u32 {
        let mut location = self.last;
        let mut count = index;
        let mut i = self.diffs.len();
        while count > 0 && i > 0 {
            count -= 1;
            i -= 1;
            let mut diff = (self.diffs[i] & 0x7F) as u32;
            while i > 0 && self.diffs[i - 1] < 0x80 {
                i -= 1;
                diff <<= 7;
                diff |= self.diffs[i] as u32;
            }
            location -= diff;
        }
        location
    }
    fn capacity(&self) -> usize {
        4+self.diffs.capacity()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn new_add_and_get_offsets() {
        let mut offsets = Locations::new(1234);
        offsets.add(1357);
        offsets.add(1470);
        offsets.add(1470);
        offsets.add(1592);
        assert_eq!(offsets.get(0), 1592);
        assert_eq!(offsets.get(1), 1470);
        assert_eq!(offsets.get(2), 1470);
        assert_eq!(offsets.get(3), 1357);
        assert_eq!(offsets.get(4), 1234);
        assert_eq!(offsets.get(8), 1234);
    }
}
