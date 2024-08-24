use crate::{
    bitarray::BitArray,
    loxtr::Loxtr,
    object::{BoundMethod, Class, Closure, Function, Instance, Upvalue, Value}, table::Table,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    BoundMethod = 1, // different (better?) miri errors
    Class,
    Closure,
    Function,
    Free,
    Instance,
    String,
    Upvalue,
}

pub trait Traceable
where
    Self: Sized,
{
    const KIND: Kind;
    fn byte_count(&self) -> usize;
    fn trace(&self, collector: &mut Vec<Handle>);
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
    string_pool: Table<()>,
    byte_count: usize,
    next_gc: usize,
}

impl Heap {
    const INIT_SIZE: usize = 1 << 12;
    pub fn new() -> Self {
        Self {
            kinds: Vec::with_capacity(Self::INIT_SIZE),
            pointers: Vec::with_capacity(Self::INIT_SIZE),
            // lengths? hmmm....
            free: Vec::with_capacity(Self::INIT_SIZE),
            string_pool: Table::new(),
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

    pub fn retain(&mut self, roots: Vec<Handle>) {
        #[cfg(feature = "log_gc")]
        let before = self.byte_count;
        #[cfg(feature = "log_gc")]
        {
            println!("-- gc begin");
            println!("byte count: {}", before);
        }
        self.sweep(self.mark(roots));
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

    fn mark(&self, mut roots: Vec<Handle>) -> BitArray {
        let mut marked = BitArray::new(self.pointers.len());

        #[cfg(feature = "log_gc")]
        {
            println!(
                "Start marking objects & tracing references. Number of roots: {}",
                roots.len()
            );
        }

        while let Some(handle) = roots.pop() {
            let index = handle.0 as usize;
            if marked.get(index) {
                continue;
            }
            marked.add(index);
            match self.kinds[index] {
                Kind::BoundMethod => self.get_ref::<BoundMethod>(handle).trace(&mut roots),
                Kind::Class => self.get_ref::<Class>(handle).trace(&mut roots),
                Kind::Closure => self.get_ref::<Closure>(handle).trace(&mut roots),
                Kind::Function => self.get_ref::<Function>(handle).trace(&mut roots),
                Kind::Free => {}
                Kind::Instance => self.get_ref::<Instance>(handle).trace(&mut roots),
                Kind::String => {}
                Kind::Upvalue => self.get_ref::<Upvalue>(handle).trace(&mut roots),
            }
        }

        #[cfg(feature = "log_gc")]
        {
            println!("Done with mark & trace");
        }
        marked
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
            Kind::Function => unsafe {
                let ptr = self.pointers[i] as *mut Function;
                self.byte_count -= &(*ptr).byte_count();
                drop(Box::from_raw(ptr));
            },
            Kind::Free => {}
            Kind::Instance => unsafe {
                let ptr = self.pointers[i] as *mut Instance;
                self.byte_count -= &(*ptr).byte_count();
                drop(Box::from_raw(ptr));
            },
            Kind::String => unsafe {
                let ptr = self.pointers[i] as *mut Loxtr;
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
        self.string_pool.sweep(marked);
        #[cfg(feature = "log_gc")]
        {
            println!("Done sweeping");
        }
    }

    pub fn increase_byte_count(&mut self, diff: usize) {
        self.byte_count += diff;
    }

    pub fn intern_copy(&mut self, name: &str) -> Handle {
        self.string_pool.add_str(name, self)
    }

    pub fn intern(&mut self, name: String) -> Handle {
        self.string_pool.add_str(&name, self)
    }

    pub fn needs_gc(&self) -> bool {
        self.byte_count > self.next_gc
    }

    pub fn kind(&self, handle: Handle) -> Kind {
        self.kinds[handle.0 as usize]
    }

    pub fn to_string(&self, handle: Handle) -> String {
        match self.kind(handle) {
            Kind::BoundMethod => self.to_string(self.get_ref::<BoundMethod>(handle).method),
            Kind::Class => format!(
                "<class {}>",
                self.get_ref::<Loxtr>(self.get_ref::<Class>(handle).name)
                    .as_ref()
            ),
            Kind::Closure => self.to_string(self.get_ref::<Closure>(handle).function),
            Kind::Function => {
                let function = self.get_ref::<Function>(handle);
                if let Some(str) = function.name {
                    format!(
                        "<fn {} ({}/{})>",
                        self.get_ref::<Loxtr>(str).as_ref(),
                        function.arity,
                        function.upvalue_count
                    )
                } else {
                    format!("<script>")
                }
            }
            Kind::Free => format!("<free>"),
            Kind::Instance => {
                let instance = self.get_ref::<Instance>(handle);
                let class = self.get_ref::<Class>(instance.class);
                format!("<{} instance>", self.get_ref::<Loxtr>(class.name).as_ref())
            }
            Kind::String => self.get_ref::<Loxtr>(handle).as_ref().to_owned(),
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
