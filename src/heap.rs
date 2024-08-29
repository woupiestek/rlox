use crate::{
    bitarray::BitArray,
    functions::Functions,
    object::{BoundMethod, Class, Closure, Instance, Upvalue, Value},
    strings::{KeySet, StringHandle, Strings},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    BoundMethod = 1, // different (better?) miri errors
    Class,
    Closure,
    Free,
    Instance,
    Upvalue,
}

pub trait Traceable
where
    Self: Sized,
{
    const KIND: Kind;
    fn byte_count(&self) -> usize;
    fn trace(&self, handles: &mut Vec<Handle>, strings: &mut Vec<StringHandle>);
    fn get(heap: &Heap, handle: Handle) -> *mut Self {
        assert_eq!(Self::KIND, heap.kinds[handle.0 as usize]);
        heap.pointers[handle.0 as usize] as *mut Self
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Handle(u32);

pub struct Heap {
    kinds: Vec<Kind>,
    pointers: Vec<*mut u8>, // why not store lengths?
    free: Vec<Handle>,
    string_pool: Strings,
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
            byte_count: 0,
            next_gc: 1 << 20,
        }
    }

    pub fn put<T: Traceable>(&mut self, t: T) -> Handle {
        self.byte_count += t.byte_count();
        if let Some(handle) = self.free.pop() {
            self.kinds[handle.0 as usize] = T::KIND;
            self.pointers[handle.0 as usize] = Box::into_raw(Box::from(t)) as *mut u8;
            handle
        } else {
            let index = self.pointers.len();
            self.pointers.push(Box::into_raw(Box::from(t)) as *mut u8);
            self.kinds.push(T::KIND);
            Handle(index as u32)
        }
    }

    fn get_star_mut<T: Traceable>(&self, handle: Handle) -> *mut T {
        assert_eq!(T::KIND, self.kinds[handle.0 as usize]);
        self.pointers[handle.0 as usize] as *mut T
    }

    pub fn get_ref<T: Traceable>(&self, handle: Handle) -> &T {
        unsafe { self.get_star_mut::<T>(handle).as_ref().unwrap() }
    }

    pub fn get_mut<T: Traceable>(&mut self, handle: Handle) -> &mut T {
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

    pub fn retain(&mut self, mut roots: Vec<Handle>, mut strings: Vec<StringHandle>) {
        #[cfg(feature = "log_gc")]
        let before = self.byte_count;
        #[cfg(feature = "log_gc")]
        {
            println!("-- gc begin");
            println!("byte count: {}", before);
        }
        let (marked, key_set) = self.mark(&mut roots, &mut strings);
        self.sweep(marked);
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

    fn mark(&self, roots: &mut Vec<Handle>, strings: &mut Vec<StringHandle>) -> (BitArray, KeySet) {
        let mut marked = BitArray::new(self.pointers.len());
        let mut key_set: KeySet = KeySet::with_capacity(self.string_pool.capacity());

        #[cfg(feature = "log_gc")]
        {
            println!(
                "Start marking objects & tracing references. Number of roots: {}",
                roots.len()
            );
        }
        while let Some(string) = strings.pop() {
            key_set.put(string)
        }
        while let Some(handle) = roots.pop() {
            let index = handle.0 as usize;
            if marked.get(index) {
                continue;
            }
            marked.add(index);
            match self.kinds[index] {
                Kind::BoundMethod => self.get_ref::<BoundMethod>(handle).trace(roots, strings),
                Kind::Class => self.get_ref::<Class>(handle).trace(roots, strings),
                Kind::Closure => self.get_ref::<Closure>(handle).trace(roots, strings),
                Kind::Free => {}
                Kind::Instance => self.get_ref::<Instance>(handle).trace(roots, strings),
                Kind::Upvalue => self.get_ref::<Upvalue>(handle).trace(roots, strings),
            }
            while let Some(string) = strings.pop() {
                key_set.put(string)
            }
        }

        #[cfg(feature = "log_gc")]
        {
            println!("Done with mark & trace");
        }
        (marked, key_set)
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
            Kind::Upvalue => unsafe {
                let ptr = self.pointers[i] as *mut Upvalue;
                self.byte_count -= &(*ptr).byte_count();
                drop(Box::from_raw(ptr));
            },
        }
        self.kinds[i] = Kind::Free;
        self.free.push(Handle(i as u32));
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
        self.byte_count > self.next_gc
    }

    pub fn kind(&self, handle: Handle) -> Kind {
        self.kinds[handle.0 as usize]
    }

    pub fn to_string(&self, handle: Handle, functions: &Functions) -> String {
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
            Kind::Upvalue => format!("<upvalue>"),
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
