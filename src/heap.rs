use std::mem;

use crate::object::Handle;
pub struct Heap {
    objects: Vec<Handle>,
    gray: Vec<Handle>,
    bytes_allocated: usize,
    next_gc: usize,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            gray: Vec::new(),
            bytes_allocated: 0,
            next_gc: 0,
        }
    }
    pub fn free_objects(&mut self) {
        while let Some(mut handle) = self.objects.pop() {
            handle.destroy();
        }
    }

    pub fn manage(&mut self, handle: Handle) {
        self.objects.push(handle);
    }

    unsafe fn trace(&mut self) {
        while let Some(mut handle) = self.gray.pop() {
            handle.trace(&mut self.gray);
            // handle could already be placed on the new list of objects...
        }
    }

    unsafe fn sweep(&mut self) {
        let mut objects: Vec<Handle> = Vec::new();
        let mut objects = mem::replace(&mut self.objects, Vec::new());
        while let Some(mut handle) = objects.pop() {
            if (*handle.obj).is_marked {
                (*handle.obj).is_marked = false;
                self.manage(handle);
            } else {
                handle.destroy();
            }
        }
    }

    pub fn collect_garbage() {
        // requires access to 'roots'
    }
}
