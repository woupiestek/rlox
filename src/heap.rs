use crate::{
    bitarray::BitArray,
    bound_methods::BoundMethods,
    classes::Classes,
    closures::Closures,
    functions::Functions,
    instances::Instances,
    strings::{StringHandle, Strings},
    upvalues::Upvalues,
};

pub struct Collector {
    pub handles: [Vec<u32>; 6],
    pub keys: Vec<StringHandle>,
    pub marks: [BitArray; 7],
}

pub const BOUND_METHOD: usize = 0;
pub const INSTANCE: usize = 1;
pub const CLASS: usize = 2;
pub const CLOSURE: usize = 3;
pub const UPVALUE: usize = 4;
pub const FUNCTION: usize = 5;
pub const STRING: usize = 6;
pub const NATIVE: usize = 7;

// todo: currently, this is reconstructed every GC cycle. Keeping it may help performance
impl Collector {
    pub fn new() -> Self {
        Self {
            handles: Default::default(),
            // resizeable, resettable arrays, length updates on collection
            marks: [
                BitArray::new(),
                BitArray::new(),
                BitArray::new(),
                BitArray::new(),
                BitArray::new(),
                BitArray::new(),
                BitArray::new(),
            ],
            keys: Vec::new(),
        }
    }

    pub fn push<const KIND: usize>(&mut self, handle: Handle<KIND>) {
        if !self.marks[KIND].has(handle.index()) {
            self.handles[KIND].push(handle.0);
        }
    }

    fn mark_and_sweep(&mut self, heap: &mut Heap) {
        #[cfg(feature = "log_gc")]
        let before = heap.byte_count();
        #[cfg(feature = "log_gc")]
        {
            println!("-- gc begin");
            println!("byte count: {}", before);
        }
        self.mark(heap);
        self.sweep(heap);
        #[cfg(feature = "log_gc")]
        {
            println!("-- gc end");
            let after = heap.byte_count();
            println!(
                "   collected {} byte (from {} to {}) next at {}",
                before - after,
                before,
                after,
                heap.next_gc
            );
        }
    }

    fn mark(&mut self, heap: &Heap) {
        #[cfg(feature = "log_gc")]
        {
            let mut count = 0;
            for i in 0..7 {
                count += self.handles[i].len();
            }
            println!(
                "Start marking objects & tracing references. Number of roots: {}",
                count
            );
        }
        loop {
            // short cirquiting can make this behave unpredictably, but that does not explain the problems
            if heap.bound_methods.mark(self)
                && heap.classes.mark(self)
                && heap.closures.mark(self)
                && heap.functions.mark(self)
                && heap.instances.mark(self)
                && heap.strings.mark(self) // somehow do the conversion key -> handle here
                && heap.upvalues.mark(self)
            {
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
        heap.strings.sweep(&self.marks[STRING]);
        heap.bound_methods.sweep(&self.marks[BOUND_METHOD]);
        heap.classes.sweep(&self.marks[CLASS]);
        heap.closures.sweep(&self.marks[CLOSURE]);
        heap.functions.sweep(&self.marks[FUNCTION]);
        heap.instances.sweep(&self.marks[INSTANCE]);
        heap.upvalues.sweep(&self.marks[UPVALUE]);
        for bit_set in &mut self.marks {
            bit_set.clear();
        }
        #[cfg(feature = "log_gc")]
        {
            println!("Done sweeping");
        }
    }
}

pub trait Pool<const KIND: usize>
where
    Self: Sized,
{
    fn byte_count(&self) -> usize;
    fn count(&self) -> usize;
    fn trace(&self, handle: Handle<KIND>, collector: &mut Collector);
    fn sweep(&mut self, marks: &BitArray);
    // indicate that the collector has no more elements of a kind
    fn mark(&self, collector: &mut Collector) -> bool {
        if collector.handles[KIND].is_empty() {
            return true;
        }
        while let Some(i) = collector.handles[KIND].pop() {
            if !collector.marks[KIND].has(i as usize) {
                collector.marks[KIND].add(i as usize);
                self.trace(Handle::from(i), collector);
            }
        }
        false
    }
}

// Handle64, Handle32, Handle16 etc. More options?
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Handle<const KIND: usize>(pub u32);

impl<const KIND: usize> Handle<KIND> {
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}

impl<const KIND: usize> From<u32> for Handle<KIND> {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

pub struct Heap {
    pub bound_methods: BoundMethods,
    pub classes: Classes,
    pub closures: Closures,
    pub functions: Functions,
    pub instances: Instances,
    pub strings: Strings,
    pub upvalues: Upvalues,
    next_gc: usize,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            bound_methods: BoundMethods::new(),
            classes: Classes::new(),
            closures: Closures::new(),
            functions: Functions::new(),
            instances: Instances::new(),
            strings: Strings::with_capacity(0),
            upvalues: Upvalues::new(),
            next_gc: 1 << 20,
        }
    }

    pub fn retain(&mut self, collector: &mut Collector) {
        collector.mark_and_sweep(self);
        self.next_gc *= 2;
    }

    pub fn needs_gc(&self) -> bool {
        self.byte_count() > self.next_gc
    }

    fn byte_count(&self) -> usize {
        self.upvalues.byte_count()
            + self.strings.byte_count()
            + self.closures.byte_count()
            + self.classes.byte_count()
            + self.instances.byte_count()
            + self.bound_methods.byte_count()
            + self.strings.byte_count()
            + self.functions.byte_count()
    }
}
