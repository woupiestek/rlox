use std::{
    alloc::{self, Layout},
    marker::PhantomData,
    mem, ptr,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Handle<T>(u32, PhantomData<T>);

impl<T> Handle<T> {
    fn new(i: u32) -> Self {
        Self(i, PhantomData::default())
    }
}

pub struct Pool<T: Sized> {
    capacity: u32,
    base_ptr: *mut T,
    used_count: u32,
    free: u32,
}

impl<T: Sized> Pool<T> {
    pub fn empty() -> Self {
        assert!(
            mem::size_of::<T>() >= 4 && mem::align_of::<T>() >= 4,
            "Pool are for >= 4 byte aligned >= 4 byte types"
        );
        Self {
            base_ptr: ptr::null_mut(),
            capacity: 0,
            free: 0,
            used_count: 0,
        }
    }

    fn grow(&mut self) {
        if self.capacity == 0 {
            let layout = Layout::array::<T>(8).unwrap();
            self.base_ptr = unsafe {
                let ptr = alloc::alloc(layout);
                assert!(!ptr.is_null(), "Out of memory");
                ptr as *mut T
            };
            self.capacity = 8;
        } else {
            let layout = Layout::array::<T>(self.capacity as usize).unwrap();
            self.base_ptr = unsafe {
                let ptr = alloc::realloc(self.base_ptr as *mut u8, layout, 2 * layout.size());
                assert!(!ptr.is_null(), "Out of memory");
                ptr as *mut T
            };
            self.capacity *= 2;
        }
    }

    pub fn get_ref(&self, handle: Handle<T>) -> &T {
        assert!(handle.0 < self.capacity);
        unsafe { &*self.get_star_mut(handle.0) }
    }

    unsafe fn get_star_mut(&self, offset: u32) -> *mut T {
        self.base_ptr.add(offset as usize)
    }

    pub fn get_mut(&self, handle: Handle<T>) -> &mut T {
        assert!(handle.0 < self.capacity);
        unsafe { &mut *self.get_star_mut(handle.0) }
    }

    pub fn acquire(&mut self) -> Handle<T> {
        let handle = Handle::<T>::new(self.free);
        if self.free < self.used_count {
            self.free = unsafe { *(self.get_star_mut(self.free) as *mut u32) };
        } else {
            self.free += 1;
            self.used_count += 1;
            if self.used_count == self.capacity {
                self.grow();
            }
        }
        unsafe {
            // zero out the offered slot now
            self.get_star_mut(handle.0)
                .write_bytes(0, mem::size_of::<T>());
        }
        return handle;
    }

    pub fn release(&mut self, handle: Handle<T>) {
        // will the previous tenant drop?
        unsafe { *(self.get_star_mut(handle.0) as *mut u32) = self.free }
        self.free = handle.0;
    }
}

impl<T> Drop for Pool<T> {
    fn drop(&mut self) {
        if self.capacity > 0 {
            let layout = Layout::array::<T>(self.capacity as usize).unwrap();
            unsafe { alloc::dealloc(self.base_ptr as *mut u8, layout) }
        }
    }
}
