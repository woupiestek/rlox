use std::fmt::Display;

pub struct Loxtr {
    hash: u64,
    chars: Box<str>,
}

pub fn hash_str(str: &str) -> u64 {
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
    pub fn take(str: String) -> Self {
        Self {
            hash: hash_str(&str),
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

#[cfg(test)]
mod tests {
    use crate::{
        heap::Heap,
        table::Table,
    };

    use super::*;

    #[test]
    pub fn loxtr_equality() {
        let mut heap = Heap::new();
        let key = heap.put(Loxtr::copy("str"));
        assert_ne!(key, heap.put(Loxtr::copy("str")));
        assert_eq!(heap.get_ref::<Loxtr>(key).as_ref(), "str");
        assert_eq!(heap.get_ref::<Loxtr>(key).hash_code(), hash_str("str"));

        let mut table = Table::new();
        table.set(key, (), &heap);
        let value = table.add_str("str", &mut heap);
        assert_eq!(value, key);
        assert_eq!(heap.intern_copy("str"), heap.intern_copy("str"));
    }
}
