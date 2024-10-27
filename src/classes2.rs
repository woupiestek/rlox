use crate::{
    bitarray::BitArray,
    closures::ClosureHandle,
    heap::{Collector, Handle, Pool, CLASS},
    strings::{StringHandle, Strings},
    u32s::U32s,
};

fn index(class: u8, method_name: StringHandle) -> usize {
    17 * 257 * 257 + class as usize * 257 + method_name.0 as usize
}

struct Batch {
    count: usize,
    classes: Box<[u8]>,
    method_names: Box<[StringHandle]>,
    closures: Box<[ClosureHandle]>,
}

impl Batch {
    fn byte_count(&self) -> usize {
        56 + self.method_names.len() * 9
    }

    fn with_capacity(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "what were you thinking?");
        Self {
            count: 0,
            classes: vec![0; capacity].into_boxed_slice(),
            method_names: vec![StringHandle::EMPTY; capacity].into_boxed_slice(),
            closures: vec![ClosureHandle::from(0); capacity].into_boxed_slice(),
        }
    }

    fn capacity(&self) -> usize {
        self.method_names.len()
    }

    fn find(&self, class: u8, method_name: StringHandle) -> (bool, usize) {
        let mask = self.capacity() - 1;

        let mut index = index(class, method_name) & mask;

        let mut tombstone: Option<usize> = None;
        loop {
            match self.method_names[index] {
                StringHandle::EMPTY => return (false, tombstone.unwrap_or(index)),
                StringHandle::TOMBSTONE => tombstone = Some(index),
                name => {
                    if name == method_name && self.classes[index] == class {
                        return (true, index);
                    }
                }
            }
            index = (index + 1) & mask;
        }
    }

    fn get(&self, class: u8, method_name: StringHandle) -> Option<ClosureHandle> {
        let (found, index) = self.find(class, method_name);
        if found {
            Some(self.closures[index])
        } else {
            None
        }
    }

    fn put(
        &mut self,
        class: u8,
        method_name: StringHandle,
        ch: ClosureHandle,
        indices: &mut Vec<u32>,
    ) {
        let (found, index) = self.find(class, method_name);
        self.closures[index] = ch;
        if found {
            return;
        }
        self.classes[index] = class;
        self.method_names[index] = method_name;
        self.count += 1;
        indices.push(index as u32);
    }
}

pub type ClassHandle = Handle<CLASS>;

impl Handle<CLASS> {
    fn batch(&self) -> usize {
        self.index() >> 8
    }

    fn class(&self) -> u8 {
        (self.0 & 0xff) as u8
    }
}

pub struct Classes {
    names: U32s,
    methods: Vec<Batch>,
    indices: Vec<Vec<u32>>,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            names: U32s::new(),
            methods: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn new_class(&mut self, name: StringHandle) -> ClassHandle {
        let i = self.names.store(name.0);
        let batch = i >> 8;
        while batch >= self.methods.len() as u32 {
            self.methods.push(Batch::with_capacity(8));
        }
        while i >= self.indices.len() as u32 {
            self.indices.push(Vec::new());
        }
        ClassHandle::from(i)
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, strings: &'s Strings) -> &'s str {
        strings.get(StringHandle(self.names.get(ch.0))).unwrap()
    }

    pub fn to_string(&self, ch: ClassHandle, strings: &Strings) -> String {
        format!("<class {}>", self.get_name(ch, strings))
    }

    pub fn get_method(&self, ch: ClassHandle, name: StringHandle) -> Option<ClosureHandle> {
        self.methods[ch.batch()].get(ch.class(), name)
    }

    fn grow(&mut self, batch: usize) {
        let mut new_batch = Batch::with_capacity(self.methods[batch].capacity() * 2);
        for class in 0..256 {
            let class_handle = (batch << 8) + class;
            if class_handle >= self.indices.len() {
                break;
            }
            let mut new_indices: Vec<u32> = Vec::new();
            for &index in &self.indices[class_handle] {
                new_batch.put(
                    class as u8,
                    self.methods[batch].method_names[index as usize],
                    self.methods[batch].closures[index as usize],
                    &mut new_indices,
                );
            }
            self.indices[class_handle] = new_indices
        }
        self.methods[batch] = new_batch;
    }

    pub fn set_method(&mut self, ch: ClassHandle, name: StringHandle, method: ClosureHandle) {
        let batch = ch.batch();
        if 4 * (self.methods[batch].count + 1) > 3 * self.methods[batch].capacity() {
            self.grow(batch);
        }
        self.methods[batch].put(ch.class(), name, method, &mut self.indices[ch.index()])
    }

    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        for i in 0..self.indices[super_class.index()].len() {
            let j = self.indices[super_class.index()][i] as usize;
            self.set_method(
                sub_class,
                self.methods[super_class.batch()].method_names[j],
                self.methods[super_class.batch()].closures[j],
            );
        }
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        let mut bc = 72 + self.names.capacity() * 4;
        for batch in &self.methods {
            bc += batch.byte_count();
        }
        for indices in &self.indices {
            bc += indices.capacity() * 4
        }
        bc
    }
    fn trace(&self, handle: Handle<CLASS>, collector: &mut Collector) {
        collector.keys.push(StringHandle(self.names.get(handle.0)));
        for &index in &self.indices[handle.index()] {
            collector
                .keys
                .push(self.methods[handle.batch()].method_names[index as usize]);
            collector.push(self.methods[handle.batch()].closures[index as usize]);
        }
    }
    fn sweep(&mut self, marks: &BitArray) {
        self.names.sweep(marks);
        for i in self.names.free_indices() {
            for &j in &self.indices[i] {
                self.methods[i >> 8].method_names[j as usize] = StringHandle::TOMBSTONE
            }
        }
    }
    fn count(&self) -> usize {
        self.names.count()
    }
}
