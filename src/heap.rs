use crate::{
    bitarray::BitArray,
    common::OBJECTS,
    functions::Functions,
    object::{BoundMethod, Class, Closure, Instance, Value},
    strings::{KeySet, StringHandle, Strings},
    upvalues::{UpvalueHandle, Upvalues},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    BoundMethod = 1, // different (better?) miri errors
    Class,
    Closure,
    Free,
    Instance,
}

pub struct Collector {
    pub objects: Vec<ObjectHandle>,
    pub strings: Vec<StringHandle>,
    pub upvalues: Vec<UpvalueHandle>,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            strings: Vec::new(),
            upvalues: Vec::new(),
        }
    }

    pub fn trace(&mut self, value: Value) {
        match value {
            Value::Object(o) => self.objects.push(o),
            Value::String(s) => self.strings.push(s),
            // Value::Function(_) => todo!(),
            // Value::Native(_) => todo!(),
            _ => (),
        }
    }
}

pub trait Traceable
where
    Self: Sized,
{
    const KIND: Kind;
    fn byte_count(&self) -> usize;
    fn trace(&self, collector:  &mut  Collector);
    fn get(heap: &Heap, handle: ObjectHandle) -> *mut Self {
        assert_eq!(Self::KIND, heap.kinds[handle.0 as usize]);
        heap.pointers[handle.0 as usize] as *mut Self
    }
}

// Handle64, Handle32, Handle16 etc. More options?
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Handle<const KIND: u8>(pub u32);

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
    string_pool: Strings,
    pub upvalues: Upvalues,
    byte_count: usize,
    next_gc: usize,
}

impl Heap {
    pub fn new(init_size: usize) -> Self {
        Self {
            kinds: Vec::with_capacity(init_size),
            pointers: Vec::with_capacity(init_size),
            free: Vec::with_capacity(init_size),
            string_pool: Strings::with_capacity(init_size),
            upvalues: Upvalues::new(),
            byte_count: 0,
            next_gc: 1 << 20,
        }
    }

    pub fn put<T: Traceable>(&mut self, t: T) -> ObjectHandle {
        self.byte_count += t.byte_count();
        if let Some(handle) = self.free.pop() {
            self.kinds[handle.0 as usize] = T::KIND;
            self.pointers[handle.0 as usize] = Box::into_raw(Box::from(t)) as *mut u8;
            handle
        } else {
            let index = self.pointers.len();
            self.pointers.push(Box::into_raw(Box::from(t)) as *mut u8);
            self.kinds.push(T::KIND);
            ObjectHandle::from(index as u32)
        }
    }

    fn get_star_mut<T: Traceable>(&self, handle: ObjectHandle) -> *mut T {
        assert_eq!(T::KIND, self.kinds[handle.0 as usize]);
        self.pointers[handle.0 as usize] as *mut T
    }

    pub fn get_ref<T: Traceable>(&self, handle: ObjectHandle) -> &T {
        unsafe { self.get_star_mut::<T>(handle).as_ref().unwrap() }
    }

    pub fn get_mut<T: Traceable>(&mut self, handle: ObjectHandle) -> &mut T {
        unsafe { self.get_star_mut::<T>(handle).as_mut().unwrap() }
    }

    pub fn try_ref<T: Traceable>(&self, value: Value) -> Option<&T> {
        if let Value::Object(handle) = value {
            if T::KIND == self.kinds[handle.0 as usize] {
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
            if T::KIND == self.kinds[handle.0 as usize] {
                unsafe { self.get_star_mut::<T>(handle).as_mut() }
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_str(&self, handle: StringHandle) -> &str {
        self.string_pool.get(handle).unwrap()
    }

    pub fn retain(&mut self, mut collector: Collector) {
        #[cfg(feature = "log_gc")]
        let before = self.byte_count;
        #[cfg(feature = "log_gc")]
        {
            println!("-- gc begin");
            println!("byte count: {}", before);
        }
        let (key_set, marked_objects, marked_upvalues ) = self.mark(&mut collector);
        self.sweep(marked_objects);
        self.upvalues.sweep(marked_upvalues);
        self.string_pool.sweep(key_set);

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

    fn mark(
        &self,
        collector: &mut Collector
    ) -> (KeySet,BitArray, BitArray) {
        let mut marked_objects = BitArray::new(self.pointers.len());
        let mut marked_upvalues = BitArray::new(self.upvalues.count());
        let mut key_set: KeySet = KeySet::with_capacity(self.string_pool.capacity());

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
                is_empty=false;
                key_set.put(string)
            }
            if let Some(handle) = collector.objects.pop() {
                is_empty = false;
                let index = handle.0 as usize;
                if marked_objects.get(index) {
                    continue;
                }
                marked_objects.add(index);
                match self.kinds[index] {
                    Kind::BoundMethod => self.get_ref::<BoundMethod>(handle).trace(collector),
                    Kind::Class => self.get_ref::<Class>(handle).trace(collector),
                    Kind::Closure => self.get_ref::<Closure>(handle).trace(collector),
                    Kind::Free => {}
                    Kind::Instance => self.get_ref::<Instance>(handle).trace(collector),
                }
            }
            if let Some(handle) = collector.upvalues.pop() {
                is_empty = false;
                if !marked_upvalues.get(handle.0 as usize) {
                    marked_upvalues.add(handle.0 as usize);
                    self.upvalues.trace(handle, collector);
                }
            }
            if is_empty { break; }
        }
        #[cfg(feature = "log_gc")]
        {
            println!("Done with mark & trace");
        }
        (key_set,marked_objects,marked_upvalues)
    }

    fn free(&mut self, i: usize) {
        match self.kinds[i] {
            Kind::BoundMethod => unsafe {
                let ptr = self.pointers[i] as *mut BoundMethod;
                self.byte_count -= &(*ptr).byte_count();
                drop(Box::from_raw(ptr));
            },
            Kind::Class => unsafe {
                let ptr = self.pointers[i] as *mut Class;
                self.byte_count -= &(*ptr).byte_count();
                drop(Box::from_raw(ptr));
            },
            Kind::Closure => unsafe {
                let ptr = self.pointers[i] as *mut Closure;
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
        self.string_pool.put(name)
    }

    pub fn concat(&mut self, a: StringHandle, b: StringHandle) -> Option<StringHandle> {
        // todo: count added bytes somehow
        self.string_pool.concat(a, b)
    }

    pub fn needs_gc(&self) -> bool {
        self.byte_count + self.upvalues.byte_count() + self.string_pool.byte_count() > self.next_gc
    }

    pub fn kind(&self, handle: ObjectHandle) -> Kind {
        self.kinds[handle.0 as usize]
    }

    pub fn to_string(&self, handle: ObjectHandle, functions: &Functions) -> String {
        match self.kind(handle) {
            Kind::BoundMethod => {
                self.to_string(self.get_ref::<BoundMethod>(handle).method, functions)
            }
            Kind::Class => format!(
                "<class {}>",
                self.string_pool
                    .get(self.get_ref::<Class>(handle).name)
                    .unwrap_or("???")
            ),
            Kind::Closure => functions.to_string(self.get_ref::<Closure>(handle).function, self),
            Kind::Free => format!("<free>"),
            Kind::Instance => {
                let instance = self.get_ref::<Instance>(handle);
                let class = self.get_ref::<Class>(instance.class);
                format!("<{} instance>", self.string_pool.get(class.name).unwrap())
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
