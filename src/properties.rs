// try the hashmap of vecs idea for instances

use std::mem;

use crate::{handles::HandleSet, heap::Traceable, symbols::SymbolHandle, values::Value};

pub struct Properties {
    instances: HandleSet,
    keys: Vec<SymbolHandle>,
    values: Vec<Vec<Value>>,
    indices: Box<[u16]>,
    value_count: usize,
}

impl Properties {
    const EMPTY: u16 = u16::MAX;

    pub fn new() -> Self {
        Self {
            instances: HandleSet::new(),
            keys: Vec::new(),
            values: Vec::new(),
            indices: vec![Self::EMPTY; 8].into_boxed_slice(),
            value_count: 0,
        }
    }

    fn find_index(&self, key: SymbolHandle) -> (bool, u16) {
        let mask = (self.indices.len() - 1) as u16;
        let mut hash = (key.0 as u16).reverse_bits() >> mask.leading_zeros();
        loop {
            let index = self.indices[hash as usize];
            if index == Self::EMPTY {
                return (false, hash as u16);
            }
            if self.keys[index as usize] == key {
                return (true, hash as u16);
            }
            hash = (hash + 1) & mask;
        }
    }

    fn index(&self, hash: u16) -> usize {
        self.indices[hash as usize] as usize
    }

    pub fn get(&self, instance: usize, key: SymbolHandle) -> Value {
        let (is_match, hash) = self.find_index(key);
        if !is_match {
            return Value::UNDEFINED;
        }
        let values = &self.values[self.index(hash)];
        if values.len() <= instance {
            Value::UNDEFINED
        } else {
            values[instance]
        }
    }

    pub fn set(&mut self, instance: usize, key: SymbolHandle, value: Value) -> bool {
        let (is_match, hash) = self.find_index(key);
        if is_match {
            let index = self.index(hash);
            let values = &mut self.values[index];
            if values.len() <= instance {
                self.value_count -= values.len();
                values.resize((instance + 1).next_power_of_two(), Value::UNDEFINED);
                self.value_count += values.len();
            }
            values[instance] = value;
            return false;
        }

        self.keys.push(key);
        let mut values = vec![Value::UNDEFINED; (instance + 1).next_power_of_two()];
        self.value_count += values.len();
        values[instance] = value;
        self.values.push(values);
        self.indices[hash as usize] = (self.keys.len() - 1) as u16;

        if self.keys.len() * 4 >= self.indices.len() * 3 {
            self.grow()
        }
        true
    }

    pub fn delete(&mut self, instance: usize, key: SymbolHandle) -> bool {
        let (is_match, hash) = self.find_index(key);
        if !is_match {
            return false;
        }
        let index = self.index(hash);
        let values = &mut self.values[index];
        if values.len() <= instance {
            return false;
        }
        values[instance] = Value::UNDEFINED;
        true
    }

    fn grow(&mut self) {
        self.indices = vec![Self::EMPTY; self.indices.len() * 2].into_boxed_slice();
        for i in 0..self.keys.len() {
            let (_, hash) = self.find_index(self.keys[i]);
            self.indices[hash as usize] = i as u16
        }
    }

    pub fn new_instance(&mut self) -> u32 {
        let instance = self.instances.next();
        for i in 0..self.values.len() {
            if (instance as usize) < self.values[i].len() {
                self.values[i][instance as usize] = Value::UNDEFINED;
            }
        }
        instance
    }

    pub fn byte_count(&self) -> usize {
        mem::size_of::<Properties>()+
        self.instances.byte_count() + // fixme: counting stuff double.
        self.indices.len()*2+
        self.keys.capacity()*4+
        self.values.capacity()*6+
        self.value_count*8
    }

    pub fn reset(&mut self) {
        self.instances.clear();
    }

    pub fn mark(&mut self, h: u32) -> bool {
        self.instances.mark(h)
    }

    pub fn trace_all(&mut self, instances: Vec<u32>, collector: &mut crate::heap::Collector) {
        for i in 0..self.values.len() {
            for &j in &instances {
                if (j as usize) < self.values[i].len() {
                    self.values[i][j as usize].trace(collector);
                }
            }
        }
    }
}
