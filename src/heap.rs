use std::mem;

use crate::{
    bitarray::BitArray,
    classes::Classes,
    closures::Closures,
    common::{CLASSES, CLOSURES, INSTANCES, OBJECTS, STRINGS, UPVALUES},
    functions::Functions,
    instances::Instances,
    object::BoundMethod,
    strings::{KeySet, StringHandle, Strings},
    upvalues::Upvalues,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    BoundMethod = 1, // different (better?) miri errors
    Free,
}

pub struct Collector {
    pub handles: [Vec<u32>; 8],
    pub marks: [BitArray; 8],
    pub strings: KeySet,
}

// todo: currently, this is reconstructed every GC cycle. Keeping it may help performance
impl Collector {
    pub fn new(heap: &Heap) -> Self {
        Self {
            handles: Default::default(),
            // resizeable, resettable arrays, length updates on collection
            marks: [
                BitArray::new(0),            //strings
                BitArray::new(0),            //natives
                BitArray::new(0),            //functions
                BitArray::new(heap.count()), //objects
                BitArray::new(heap.upvalues.count()),
                BitArray::new(heap.closures.count()),
                BitArray::new(heap.classes.count()),
                BitArray::new(heap.instances.count()),
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
            if let Some(i) = self.handles[STRINGS as usize].pop() {
                self.strings.put(Handle::from(i));
                done = false
            }
            done &= heap.mark(self)
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
        heap.classes.sweep(&self.marks[CLASSES as usize]);
        heap.closures.sweep(&self.marks[CLOSURES as usize]);
        heap.sweep(&self.marks[OBJECTS as usize]);
        // this is pain.
        heap.strings
            .sweep(mem::replace(&mut self.strings, KeySet::with_capacity(0)));
        heap.upvalues.sweep(&self.marks[UPVALUES as usize]);
        heap.instances.sweep(&self.marks[INSTANCES as usize]);
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

pub trait Traceable
where
    Self: Sized,
{
    const KIND: Kind;
    fn byte_count(&self) -> usize;
    fn trace(&self, collector: &mut Collector);
    fn get(heap: &Heap, handle: ObjectHandle) -> *mut Self {
        assert_eq!(Self::KIND, heap.kinds[handle.index()]);
        heap.pointers[handle.index()] as *mut Self
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

pub type ObjectHandle = Handle<OBJECTS>;

pub struct Heap {
    kinds: Vec<Kind>,
    pointers: Vec<*mut u8>, // why not store lengths?
    free: Vec<ObjectHandle>,
    pub strings: Strings,
    pub upvalues: Upvalues,
    pub closures: Closures,
    pub classes: Classes,
    pub instances: Instances,
    _byte_count: usize,
    next_gc: usize,
}

impl Heap {
    pub fn new(init_size: usize) -> Self {
        Self {
            kinds: Vec::with_capacity(init_size),
            pointers: Vec::with_capacity(init_size),
            free: Vec::with_capacity(init_size),
            strings: Strings::with_capacity(init_size),
            upvalues: Upvalues::new(),
            closures: Closures::new(),
            classes: Classes::new(),
            instances: Instances::new(),
            _byte_count: 0,
            next_gc: 1 << 20,
        }
    }

    pub fn put<T: Traceable>(&mut self, t: T) -> ObjectHandle {
        self._byte_count += t.byte_count();
        if let Some(handle) = self.free.pop() {
            self.kinds[handle.index()] = T::KIND;
            self.pointers[handle.index()] = Box::into_raw(Box::from(t)) as *mut u8;
            handle
        } else {
            let index = self.pointers.len();
            self.pointers.push(Box::into_raw(Box::from(t)) as *mut u8);
            self.kinds.push(T::KIND);
            ObjectHandle::from(index as u32)
        }
    }

    fn get_star_mut<T: Traceable>(&self, handle: ObjectHandle) -> *mut T {
        assert_eq!(T::KIND, self.kinds[handle.index()]);
        self.pointers[handle.index()] as *mut T
    }

    pub fn get_ref<T: Traceable>(&self, handle: ObjectHandle) -> &T {
        unsafe { self.get_star_mut::<T>(handle).as_ref().unwrap() }
    }

    pub fn get_str(&self, handle: StringHandle) -> &str {
        self.strings.get(handle).unwrap()
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

    fn free(&mut self, i: usize) {
        match self.kinds[i] {
            Kind::BoundMethod => unsafe {
                let ptr = self.pointers[i] as *mut BoundMethod;
                self._byte_count -= &(*ptr).byte_count();
                drop(Box::from_raw(ptr));
            },
            Kind::Free => {}
        }
        self.kinds[i] = Kind::Free;
        self.free.push(ObjectHandle::from(i as u32));
    }

    pub fn intern_copy(&mut self, name: &str) -> StringHandle {
        self._byte_count += name.len();
        self.strings.put(name)
    }

    pub fn concat(&mut self, a: StringHandle, b: StringHandle) -> Option<StringHandle> {
        // todo: count added bytes somehow
        self.strings.concat(a, b)
    }

    pub fn needs_gc(&self) -> bool {
        self._byte_count
            + self.upvalues.byte_count()
            + self.strings.byte_count()
            + self.closures.byte_count()
            + self.classes.byte_count()
            + self.instances.byte_count()
            > self.next_gc
    }

    pub fn kind(&self, handle: ObjectHandle) -> Kind {
        self.kinds[handle.index()]
    }

    pub fn to_string(&self, handle: ObjectHandle, functions: &Functions) -> String {
        match self.kind(handle) {
            Kind::BoundMethod => functions.to_string(
                self.closures
                    .function_handle(self.get_ref::<BoundMethod>(handle).method),
                self,
            ),
            Kind::Free => format!("<free>"),
        }
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        for i in 0..self.pointers.len() {
            self.free(i);
        }
    }
}

impl Pool<OBJECTS> for Heap {
    fn trace(&self, handle: ObjectHandle, collector: &mut Collector) {
        match self.kinds[handle.index()] {
            Kind::BoundMethod => self.get_ref::<BoundMethod>(handle).trace(collector),
            Kind::Free => {}
        }
    }

    fn sweep(&mut self, marked: &BitArray) {
        for i in 0..self.pointers.len() {
            if !marked.get(i) {
                self.free(i);
            }
        }
    }

    fn byte_count(&self) -> usize {
        self._byte_count
    }

    fn count(&self) -> usize {
        self.pointers.len()
    }
}
