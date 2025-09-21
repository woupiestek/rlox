use crate::{
    closures::ClosureHandle,
    handles::Handles,
    heap::{Collector, Handle, Pool, CLASS},
    strings::{StringHandle, Strings},
};

fn index(i: u8, key: StringHandle) -> u32 {
    (i as u32 ^ key.0).wrapping_mul(16777619u32)
}

// another hash map...
struct Batch {
    count: usize,
    classes: Box<[u8]>,
    keys: Box<[StringHandle]>,
    closures: Box<[ClosureHandle]>,
}

impl Batch {
    fn byte_count(&self) -> usize {
        56 + self.keys.len() * 9
    }

    fn with_capacity(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "what were you thinking?");
        Self {
            count: 0,
            classes: vec![0; capacity].into_boxed_slice(),
            keys: vec![StringHandle::EMPTY; capacity].into_boxed_slice(),
            closures: vec![ClosureHandle::from(0); capacity].into_boxed_slice(),
        }
    }

    fn capacity(&self) -> usize {
        self.keys.len()
    }

    fn find(&self, class: u8, method_name: StringHandle) -> (bool, u32) {
        assert!(4 * self.count <= 3 * self.capacity());
        let mask = (self.capacity() - 1) as u32;

        let mut index = index(class, method_name) & mask;

        let mut tombstone: u32 = u32::MAX;
        loop {
            match self.keys[index as usize] {
                StringHandle::EMPTY => {
                    return (
                        false,
                        if tombstone < u32::MAX {
                            tombstone
                        } else {
                            index
                        },
                    )
                }
                StringHandle::TOMBSTONE => tombstone = index,
                name => {
                    if name == method_name && self.classes[index as usize] == class {
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
            Some(self.closures[index as usize])
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
        self.closures[index as usize] = ch;
        if found {
            return;
        }
        self.classes[index as usize] = class;
        self.keys[index as usize] = method_name;
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
    byte_count: usize,
    names: Vec<StringHandle>,
    methods: Vec<Batch>,
    indices: Vec<Vec<u32>>,
    handles: Handles,
}

impl Classes {
    pub fn new() -> Self {
        Self {
            byte_count: 80,
            names: Vec::new(),
            methods: Vec::new(),
            indices: Vec::new(),
            handles: Handles::new(),
        }
    }

    pub fn new_class(&mut self, name: StringHandle) -> ClassHandle {
        let i = self.handles.next() as usize; //self.names.store(name.0) as usize;

        while self.names.len() <= i {
            self.names.push(StringHandle::EMPTY);
        }
        self.names[i] = name;

        let batch = i >> 8;
        if i < self.indices.len() {
            // just-in-time method clean up
            for &j in &self.indices[i] {
                self.methods[batch].keys[j as usize] = StringHandle::TOMBSTONE
            }
            self.indices[i].clear();
            return ClassHandle::from(i as u32);
        }
        while batch >= self.methods.len() {
            let batch = Batch::with_capacity(8);
            self.byte_count += batch.byte_count();
            self.methods.push(batch);
        }
        while i as usize >= self.indices.len() {
            self.indices.push(Vec::new());
        }
        ClassHandle::from(i as u32)
    }

    pub fn get_name<'s>(&self, ch: ClassHandle, strings: &'s Strings) -> &'s str {
        strings.get(self.names[ch.index()]).unwrap()
    }

    pub fn to_string(&self, ch: ClassHandle, strings: &Strings) -> String {
        format!("<class {}>", self.get_name(ch, strings))
    }

    pub fn get_method(&self, ch: ClassHandle, name: StringHandle) -> Option<ClosureHandle> {
        self.methods[ch.batch()].get(ch.class(), name)
    }

    fn grow(&mut self, batch: usize) {
        let old_batch = &self.methods[batch];
        let mut new_batch = Batch::with_capacity(old_batch.capacity() * 2);
        for class in 0..256 {
            let class_handle = (batch << 8) + class;
            if class_handle >= self.indices.len() {
                break;
            }
            let mut new_indices: Vec<u32> = Vec::new();
            for &index in &self.indices[class_handle] {
                new_batch.put(
                    class as u8,
                    old_batch.keys[index as usize],
                    old_batch.closures[index as usize],
                    &mut new_indices,
                );
            }
            self.byte_count += 4 * (new_indices.capacity() + self.indices[class_handle].capacity());
            self.indices[class_handle] = new_indices
        }
        self.byte_count += new_batch.byte_count() - old_batch.byte_count();
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
                self.methods[super_class.batch()].keys[j],
                self.methods[super_class.batch()].closures[j],
            );
        }
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        self.byte_count + self.indices.capacity() * 24 + self.names.capacity() * 4
    }
    fn trace(&mut self, handle: Handle<CLASS>, collector: &mut Collector) {
        if !self.handles.mark(handle.0) {
            return;
        }
        collector.push(self.names[handle.index()]);
        for &index in &self.indices[handle.index()] {
            collector.push(self.methods[handle.batch()].keys[index as usize]);
            collector.push(self.methods[handle.batch()].closures[index as usize]);
        }
    }

    fn reset(&mut self) {
        self.handles.clear();
    }
    fn sweep(&mut self) {}
}
