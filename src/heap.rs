use std::mem;

use crate::object::Handle;
struct Heap {
    objects: Vec<Handle>,
    gray: Vec<Handle>,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            gray: Vec::new(),
        }
    }
    pub fn freeObjects(&mut self) {
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

    pub fn collectGarbage() {
        // requires access to 'roots'
    }
}
