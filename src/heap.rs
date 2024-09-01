use std::mem;

use crate::{
    bitarray::BitArray,
    bound_methods::BoundMethods,
    classes::Classes,
    closures::Closures,
    instances::Instances,
    strings::{KeySet, StringHandle, Strings},
    upvalues::Upvalues,
};

pub struct Collector {
    pub handles: [Vec<u32>; 5],
    pub marks: [BitArray; 5],
    pub strings: KeySet,
}

#[repr(u8)]
pub enum Kind {
    BoundMethod,
    Instance,
    Class,
    Closure,
    Upvalue,
    String,
    Native,
    Function,
}

// todo: currently, this is reconstructed every GC cycle. Keeping it may help performance
impl Collector {
    pub fn new(heap: &Heap) -> Self {
        Self {
            handles: Default::default(),
            // resizeable, resettable arrays, length updates on collection
            marks: [
                BitArray::new(heap.bound_methods.count()),
                BitArray::new(heap.instances.count()),
                BitArray::new(heap.classes.count()),
                BitArray::new(heap.closures.count()),
                BitArray::new(heap.upvalues.count()),
            ],
            strings: KeySet::with_capacity(heap.strings.capacity()),
        }
    }

    pub fn push<const KIND: u8>(&mut self, handle: Handle<KIND>) {
        if !self.marks[KIND as usize].get(handle.index()) {
            self.handles[KIND as usize].push(handle.0);
        }
    }

    fn mark_and_sweep(&mut self, heap: &mut Heap) {
        self.mark(heap);
        self.sweep(heap);
    }

    fn mark(&mut self, heap: &mut Heap) {
        #[cfg(feature = "log_gc")]
        {
            println!(
                "Start marking objects & tracing references. Number of roots: {}",
                roots.len()
            );
        }
        loop {
            let mut done = true;
            if let Some(i) = self.handles[Kind::String as usize].pop() {
                self.strings.put(Handle::from(i));
                done = false
            }
            done &= heap.bound_methods.mark(self)
                && heap.classes.mark(self)
                && heap.closures.mark(self)
                && heap.instances.mark(self)
                && heap.upvalues.mark(self);
            if done {
                break;
            }
        }
        #[cfg(feature = "log_gc")]
        {
            println!("Done with mark & trace");
        }
    }

    fn sweep(&mut self, heap: &mut Heap) {
        #[cfg(feature = "log_gc")]
        {
            println!("Start sweeping.");
        }
        heap.classes.sweep(&self.marks[Kind::Class as usize]);
        heap.closures
            .sweep(&self.marks[{ Kind::Closure as u8 } as usize]);
        heap.bound_methods
            .sweep(&self.marks[{ Kind::BoundMethod as u8 } as usize]);
        // this is pain.
        heap.strings
            .sweep(mem::replace(&mut self.strings, KeySet::with_capacity(0)));
        heap.upvalues.sweep(&self.marks[Kind::Upvalue as usize]);
        heap.instances.sweep(&self.marks[Kind::Instance as usize]);
        #[cfg(feature = "log_gc")]
        {
            println!("Done sweeping");
        }
    }
}

pub trait Pool<const KIND: u8>
where
    Self: Sized,
{
    fn byte_count(&self) -> usize;
    fn count(&self) -> usize;
    fn trace(&self, handle: Handle<KIND>, collector: &mut Collector);
    fn sweep(&mut self, marks: &BitArray);
    // indicate that the collector has no more elements of a kind
    fn mark(&mut self, collector: &mut Collector) -> bool {
        if let Some(i) = collector.handles[KIND as usize].pop() {
            if !collector.marks[KIND as usize].get(i as usize) {
                self.trace(Handle::from(i), collector);
            }
            false
        } else {
            true
        }
    }
}

// Handle64, Handle32, Handle16 etc. More options?
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Handle<const KIND: u8>(pub u32);

impl<const KIND: u8> Handle<KIND> {
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}

impl<const KIND: u8> From<u32> for Handle<KIND> {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

pub struct Heap {
    pub strings: Strings,
    pub upvalues: Upvalues,
    pub closures: Closures,
    pub classes: Classes,
    pub instances: Instances,
    pub bound_methods: BoundMethods,
    next_gc: usize,
}

impl Heap {
    pub fn new(init_size: usize) -> Self {
        Self {
            strings: Strings::with_capacity(init_size),
            upvalues: Upvalues::new(),
            closures: Closures::new(),
            classes: Classes::new(),
            instances: Instances::new(),
            bound_methods: BoundMethods::new(),
            next_gc: 1 << 20,
        }
    }

    pub fn retain(&mut self, mut collector: Collector) {
        #[cfg(feature = "log_gc")]
        let before = self.byte_count;
        #[cfg(feature = "log_gc")]
        {
            println!("-- gc begin");
            println!("byte count: {}", before);
        }
        collector.mark_and_sweep(self);
        self.next_gc *= 2;
        #[cfg(feature = "log_gc")]
        {
            println!("-- gc end");
            let after = self.byte_count;
            println!(
                "   collected {} byte (from {} to {}) next at {}",
                before - after,
                before,
                after,
                self.next_gc
            );
        }
    }

    pub fn concat(&mut self, a: StringHandle, b: StringHandle) -> Option<StringHandle> {
        // todo: count added bytes somehow
        self.strings.concat(a, b)
    }

    pub fn needs_gc(&self) -> bool {
        self.upvalues.byte_count()
            + self.strings.byte_count()
            + self.closures.byte_count()
            + self.classes.byte_count()
            + self.instances.byte_count()
            + self.bound_methods.byte_count()
            + self.strings.byte_count()
            > self.next_gc
    }
}
