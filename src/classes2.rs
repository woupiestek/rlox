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
    first: [u32; 256],
    classes: Box<[u8]>,
    method_names: Box<[StringHandle]>,
    closures: Box<[ClosureHandle]>,
    next: Box<[u32]>,
}

impl Batch {
    fn byte_count(&self) -> usize {
        1096 + self.method_names.len() * 13
    }

    fn trace(&self, class: u8, collector: &mut Collector) {
        let mut index = self.first[class as usize];
        while index < u32::MAX {
            collector.keys.push(self.method_names[index as usize]);
            collector.push(self.closures[index as usize]);
            index = self.next[index as usize]
        }
    }

    fn with_capacity(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "what were you thinking?");
        Self {
            count: 0,
            first: [u32::MAX; 256],
            classes: vec![0; capacity].into_boxed_slice(),
            method_names: vec![StringHandle::EMPTY; capacity].into_boxed_slice(),
            closures: vec![ClosureHandle::from(0); capacity].into_boxed_slice(),
            next: vec![u32::MAX; capacity].into_boxed_slice(),
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

    fn grow(&self) -> Self {
        let mut greater = Self::with_capacity(self.capacity() * 2);
        for class in 0..=255 {
            let mut index = self.first[class as usize];
            while index < u32::MAX {
                greater.put(
                    class,
                    self.method_names[index as usize],
                    self.closures[index as usize],
                );
                index = self.next[index as usize];
            }
        }
        greater
    }

    fn get(&self, class: u8, method_name: StringHandle) -> Option<ClosureHandle> {
        let (found, index) = self.find(class, method_name);
        if found {
            Some(self.closures[index])
        } else {
            None
        }
    }

    fn put(&mut self, class: u8, method_name: StringHandle, ch: ClosureHandle) {
        let (found, index) = self.find(class, method_name);
        self.closures[index] = ch;
        if found {
            return;
        }
        self.classes[index] = class;
        self.method_names[index] = method_name;
        self.count += 1;
        self.next[index] = self.first[class as usize];
        self.first[class as usize] = index as u32;
    }

    fn delete_class(&mut self, class: u8) {
        let mut index = self.first[class as usize];
        self.first[class as usize] = u32::MAX;
        while index < u32::MAX {
            let j = index as usize;
            self.method_names[j] = StringHandle::TOMBSTONE;
            index = self.next[j];
            self.next[j] = u32::MAX;
        }
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
}

impl Classes {
    pub fn new() -> Self {
        Self {
            names: U32s::new(),
            methods: Vec::new(),
        }
    }

    pub fn new_class(&mut self, name: StringHandle) -> ClassHandle {
        let i = self.names.store(name.0);
        let batch = i >> 8;
        while batch >= self.methods.len() as u32 {
            self.methods.push(Batch::with_capacity(8));
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

    pub fn set_method(&mut self, ch: ClassHandle, name: StringHandle, method: ClosureHandle) {
        let batch = ch.batch();
        if 4 * (self.methods[batch].count + 1) > 3 * self.methods[batch].capacity() {
            self.methods[batch] = self.methods[batch].grow();
        }
        self.methods[batch].put(ch.class(), name, method);
    }

    pub fn clone_methods(&mut self, super_class: ClassHandle, sub_class: ClassHandle) {
        let mut index = self.methods[super_class.batch()].first[super_class.class() as usize];
        while index < u32::MAX {
            let method_name = self.methods[super_class.batch()].method_names[index as usize];
            let ch = self.methods[super_class.batch()].closures[index as usize];
            self.methods[sub_class.batch()].put(sub_class.class(), method_name, ch);
            index = self.methods[super_class.batch()].next[index as usize]
        }
    }
}

impl Pool<CLASS> for Classes {
    fn byte_count(&self) -> usize {
        let mut bc = self.names.capacity() * 4;
        for batch in &self.methods {
            bc += batch.byte_count();
        }
        bc
    }
    fn trace(&self, handle: Handle<CLASS>, collector: &mut Collector) {
        collector.keys.push(StringHandle(self.names.get(handle.0)));
        self.methods[handle.batch()].trace(handle.class(), collector);
    }
    fn sweep(&mut self, marks: &BitArray) {
        self.names.sweep(marks);
        for i in self.names.free_indices() {
            self.methods[i >> 8].delete_class((i & 0xff) as u8);
        }
    }
    fn count(&self) -> usize {
        self.names.count()
    }
}
