use std::fmt::Display;

pub struct Loxtr {
    hash: u64,
    chars: Box<str>,
}

fn hash_str(str: &str) -> u64 {
    let mut hash = 14695981039346656037u64;
    for &byte in str.as_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

impl Loxtr {
    pub fn copy(str: &str) -> Self {
        Self {
            hash: hash_str(str),
            chars: Box::from(str),
        }
    }
    pub fn hash_code(&self) -> u64 {
        self.hash
    }
}

impl AsRef<str> for Loxtr {
    fn as_ref(&self) -> &str {
        &self.chars
    }
}

impl Display for Loxtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.chars.fmt(f)
    }
}
