use crate::{
    bound_methods::BoundMethods, classes::Classes, closures::Closures, functions::Functions,
    instances::Instances, strings::Strings, symbols::Symbols, upvalues::Upvalues,
};

pub trait Traceable {
    fn trace(&self, collector: &mut Collector);
}

pub struct Collector {
    pub handles: [Vec<u32>; 8],
}

pub const BOUND_METHOD: usize = 0;
pub const CLASS: usize = 1;
pub const CLOSURE: usize = 2;
pub const FUNCTION: usize = 3;
pub const INSTANCE: usize = 4;
pub const STRING: usize = 5;
pub const SYMBOL: usize = 6;
pub const UPVALUE: usize = 7;

impl Collector {
    pub fn new() -> Self {
        Self {
            // grey set?
            handles: Default::default(),
        }
    }

    pub fn push(&mut self, kind: usize, handle: u32) {
        self.handles[kind].push(handle);
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

    fn mark(&mut self, heap: &mut Heap) {
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
        let mut marked = Vec::new();
        loop {
            // short cirquiting can make this behave unpredictably
            let mut done = true;
            done = heap.bound_methods.mark_all(&mut marked, self) && done;
            done = heap.classes.mark_all(&mut marked, self) && done;
            done = heap.closures.mark_all(&mut marked, self) && done;
            done = heap.functions.mark_all(&mut marked, self) && done;
            done = heap.instances.mark_all(&mut marked, self) && done;
            done = heap.strings.mark_all(&mut marked, self) && done;
            done = heap.symbols.mark_all(&mut marked, self) && done;
            done = heap.upvalues.mark_all(&mut marked, self) && done;

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
        heap.bound_methods.sweep();
        heap.classes.sweep();
        heap.closures.sweep();
        heap.functions.sweep();
        heap.instances.sweep();
        heap.strings.sweep();
        heap.symbols.sweep();
        heap.upvalues.sweep();
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
    // is this even still needed!?
    // fn count(&self) -> usize;
    fn reset(&mut self);
    fn sweep(&mut self);
    fn mark(&mut self, handle: u32) -> bool;
    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector);

    // indicate that the collector has no more elements of a kind
    fn mark_all(&mut self, marked: &mut Vec<u32>, collector: &mut Collector) -> bool {
        if collector.handles[KIND].is_empty() {
            return true;
        }
        marked.clear();
        while let Some(handle) = collector.handles[KIND].pop() {
            if self.mark(handle) {
                marked.push(handle);
            }
        }
        self.trace_all(marked, collector);
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

// this worries me
impl<const KIND: usize> Default for Handle<KIND> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<const KIND: usize> Traceable for Handle<KIND> {
    fn trace(&self, collector: &mut Collector) {
        collector.push(KIND, self.0);
    }
}

pub struct Heap {
    pub bound_methods: BoundMethods,
    pub classes: Classes,
    pub closures: Closures,
    pub functions: Functions,
    pub instances: Instances,
    pub strings: Strings,
    pub symbols: Symbols,
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
            strings: Strings::new(),
            symbols: Symbols::new(),
            upvalues: Upvalues::new(),
            next_gc: 1 << 20,
        }
    }

    pub fn reset(&mut self) {
        self.bound_methods.reset();
        self.classes.reset();
        self.closures.reset();
        self.functions.reset();
        self.instances.reset();
        self.strings.reset();
        self.symbols.reset();
        self.upvalues.reset();
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
            + self.symbols.byte_count()
            + self.closures.byte_count()
            + self.classes.byte_count()
            + self.instances.byte_count()
            + self.bound_methods.byte_count()
            + self.functions.byte_count()
    }
}
