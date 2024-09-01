use crate::{
    bitarray::BitArray,
    classes::{ClassHandle, Classes},
    closures::{ClosureHandle, Closures},
    common::OBJECTS,
    functions::Functions,
    object::{BoundMethod, Instance, Value},
    strings::{KeySet, StringHandle, Strings},
    upvalues::{UpvalueHandle, Upvalues},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    BoundMethod = 1, // different (better?) miri errors
    Free,
    Instance,
}

pub struct Collector {
    pub objects: Vec<ObjectHandle>,
    pub strings: Vec<StringHandle>,
    pub upvalues: Vec<UpvalueHandle>,
    pub closures: Vec<ClosureHandle>,
    pub classes: Vec<ClassHandle>,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            strings: Vec::new(),
            upvalues: Vec::new(),
            closures: Vec::new(),
            classes: Vec::new(),
        }
    }

    pub fn trace(&mut self, value: Value) {
        match value {
            Value::Object(o) => self.objects.push(o),
            Value::String(s) => self.strings.push(s),
            Value::Closure(c) => self.closures.push(c),
            Value::Class(c) => self.classes.push(c),
            // Value::Function(_) => todo!(),
            // Value::Native(_) => todo!(),
            _ => (),
        }
    }
}

pub struct Marks {
    pub objects: BitArray,
    pub strings: KeySet,
    pub upvalues: BitArray,
    pub closures: BitArray,
    pub classes: BitArray,
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
    byte_count: usize,
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
            byte_count: 0,
            next_gc: 1 << 20,
        }
    }

    pub fn put<T: Traceable>(&mut self, t: T) -> ObjectHandle {
        self.byte_count += t.byte_count();
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

    pub fn try_ref<T: Traceable>(&self, value: Value) -> Option<&T> {
        if let Value::Object(handle) = value {
            if T::KIND == self.kinds[handle.index()] {
                unsafe { self.get_star_mut::<T>(handle).as_ref() }
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn try_mut<T: Traceable>(&self, value: Value) -> Option<&mut T> {
        if let Value::Object(handle) = value {
            if T::KIND == self.kinds[handle.index()] {
                unsafe { self.get_star_mut::<T>(handle).as_mut() }
            } else {
                None
            }
        } else {
            None
        }
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
        let marks = self.mark(&mut collector);
        self.sweep(marks.objects);
        self.upvalues.sweep(marks.upvalues);
        self.strings.sweep(marks.strings);
        self.closures.sweep(marks.closures);
        self.classes.sweep(marks.classes);

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

    fn mark(&self, collector: &mut Collector) -> Marks {
        let mut marks = Marks {
            objects: BitArray::new(self.pointers.len()),
            upvalues: BitArray::new(self.upvalues.count()),
            closures: BitArray::new(self.closures.count()),
            classes: BitArray::new(self.classes.count()),
            strings: KeySet::with_capacity(self.strings.capacity()),
        };
        #[cfg(feature = "log_gc")]
        {
            println!(
                "Start marking objects & tracing references. Number of roots: {}",
                roots.len()
            );
        }
        loop {
            let mut is_empty = true;
            if let Some(string) = collector.strings.pop() {
                is_empty = false;
                marks.strings.put(string)
            }
            if let Some(handle) = collector.objects.pop() {
                is_empty = false;
                let index = handle.0 as usize;
                if marks.objects.get(index) {
                    continue;
                }
                marks.objects.add(index);
                match self.kinds[index] {
                    Kind::BoundMethod => self.get_ref::<BoundMethod>(handle).trace(collector),
                    Kind::Free => {}
                    Kind::Instance => self.get_ref::<Instance>(handle).trace(collector),
                }
            }
            if let Some(handle) = collector.upvalues.pop() {
                is_empty = false;
                if !marks.upvalues.get(handle.0 as usize) {
                    marks.upvalues.add(handle.0 as usize);
                    self.upvalues.trace(handle, collector);
                }
            }
            if let Some(handle) = collector.closures.pop() {
                is_empty = false;
                if !marks.closures.get(handle.0 as usize) {
                    marks.closures.add(handle.0 as usize);
                    self.closures.trace(handle, collector)
                }
            }
            if let Some(handle) = collector.classes.pop() {
                is_empty = false;
                if !marks.classes.get(handle.0 as usize) {
                    marks.classes.add(handle.0 as usize);
                    self.classes.trace(handle, collector)
                }
            }
            if is_empty {
                break;
            }
        }
        #[cfg(feature = "log_gc")]
        {
            println!("Done with mark & trace");
        }
        marks
    }

    fn free(&mut self, i: usize) {
        match self.kinds[i] {
            Kind::BoundMethod => unsafe {
                let ptr = self.pointers[i] as *mut BoundMethod;
                self.byte_count -= &(*ptr).byte_count();
                drop(Box::from_raw(ptr));
            },
            Kind::Free => {}
            Kind::Instance => unsafe {
                let ptr = self.pointers[i] as *mut Instance;
                self.byte_count -= &(*ptr).byte_count();
                drop(Box::from_raw(ptr));
            },
        }
        self.kinds[i] = Kind::Free;
        self.free.push(ObjectHandle::from(i as u32));
    }

    fn sweep(&mut self, marked: BitArray) {
        #[cfg(feature = "log_gc")]
        {
            println!("Start sweeping.");
        }
        for i in 0..self.pointers.len() {
            if !marked.get(i) {
                self.free(i);
            }
        }
        #[cfg(feature = "log_gc")]
        {
            println!("Done sweeping");
        }
    }

    pub fn increase_byte_count(&mut self, diff: usize) {
        self.byte_count += diff;
    }

    pub fn intern_copy(&mut self, name: &str) -> StringHandle {
        self.byte_count += name.len();
        self.strings.put(name)
    }

    pub fn concat(&mut self, a: StringHandle, b: StringHandle) -> Option<StringHandle> {
        // todo: count added bytes somehow
        self.strings.concat(a, b)
    }

    pub fn needs_gc(&self) -> bool {
        self.byte_count
            + self.upvalues.byte_count()
            + self.strings.byte_count()
            + self.closures.byte_count()
            + self.classes.byte_code()
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
            Kind::Instance => {
                let instance = self.get_ref::<Instance>(handle);
                format!(
                    "<{} instance>",
                    self.classes.get_name(instance.class, &self.strings)
                )
            }
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
