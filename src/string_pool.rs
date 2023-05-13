// deviate to get a small part correct
use std::mem;

#[derive(Debug, PartialEq)]
pub struct InternedString {
    hash: u32,
    mark: bool,
    string: String,
}

pub fn hash(chars: &str) -> u32 {
    let bytes = chars.as_bytes();
    let mut hash = 2166136261u32;
    for i in 0..chars.len() {
        hash ^= bytes[i] as u32;
        hash *= 16777619;
    }
    return hash;
}

type Entries = Vec<Option<InternedString>>;

pub struct StringPool {
    count: u32,
    entries: Entries,
}

// a lot to figure out here
impl StringPool {
    pub fn new() -> Self {
        Self {
            count: 0,
            entries: Vec::new(),
        }
    }

    const MAX_LOAD: f32 = 0.75;

    fn find(&self, key: &str, hash: usize) -> usize {
        let mask = self.entries.capacity() - 1;
        let mut index = hash & mask;
        loop {
            match &self.entries[index] {
                None => {
                    return index;
                }
                Some(interned_string) => {
                    if interned_string.string == key {
                        return index;
                    }
                    index = (index + 1) & mask;
                }
            }
        }
    }

    fn grow(&mut self, capacity: usize) {
        let entries = mem::replace(&mut self.entries, (0..capacity).map(|_| None).collect());
        self.count = 0;
        for entry in entries {
            if let Some(interned_string) = &entry {
                let index = self.find(&interned_string.string, interned_string.hash as usize);
                self.entries[index] = entry;
            }
        }
    }

    pub fn copy(&mut self, key: &str) -> &InternedString {
        self.take(String::from(key))
    }

    pub fn take(&mut self, string: String) -> &InternedString {
        if (self.count + 1) as f32 > self.entries.capacity() as f32 * StringPool::MAX_LOAD {
            self.grow(if self.entries.capacity() < 8 {
                8
            } else {
                self.entries.capacity() * 2
            });
        }
        let hash = hash(&string);
        let index = self.find(&string, hash as usize);
        self.entries[index].get_or_insert_with(|| InternedString {
            hash,
            mark: false,
            string,
        })
    }

    pub fn mark(self) {
        for entry in self.entries {
            if let Some(mut interned_string) = entry {
                interned_string.mark = true;
            }
        }
    }

    pub fn sweep(&mut self) {
        for index in 0..self.entries.len() {
            if let Some(interned_string) = &self.entries[index] {
                if interned_string.mark == true {
                    self.entries[index] = None;
                }
            }
        }
    }
}
