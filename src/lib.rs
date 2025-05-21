use std::alloc::{GlobalAlloc, Layout};
use std::cmp;
use std::ffi::c_void;
use std::ptr;

#[allow(dead_code)]
pub mod bindings {
    use std::ffi::c_void;
    use std::os::raw::c_int;
    extern "C" {
        pub fn TCMallocInternalMalloc(size: usize) -> *mut c_void;
        pub fn TCMallocInternalCalloc(num: usize, size: usize) -> *mut c_void;
        pub fn TCMallocInternalRealloc(ptr: *mut c_void, size: usize) -> *mut c_void;
        pub fn TCMallocInternalFree(ptr: *mut c_void);
        pub fn TCMallocInternalMemalign(alignment: usize, size: usize) -> *mut c_void;
        pub fn TCMallocInternalSdallocx(ptr: *mut c_void, size: usize, flags: c_int) -> ();
        pub fn TCMallocInternalFreeSized(ptr: *mut c_void, size: usize);

        pub fn TCMallocInternalMallocSize(ptr: *mut c_void) -> usize;
    }
}

const TCMALLOC_DEFAULT_ALIGN: usize = 16;

#[inline]
fn needs_custom_alignment(align: usize, _size: usize) -> bool {
    align > TCMALLOC_DEFAULT_ALIGN
}

/// Returns a pointer suitable for a zero-sized allocation with the given alignment.
/// It must be non-null and aligned to `align`.
#[inline]
fn zero_sized_dangling_ptr(align: usize) -> *mut u8 {
    let actual_align = cmp::max(align, 1);
    actual_align as *mut u8
}

pub struct TcMalloc;

unsafe impl GlobalAlloc for TcMalloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return zero_sized_dangling_ptr(layout.align());
        }

        let ptr = if needs_custom_alignment(layout.align(), layout.size()) {
            bindings::TCMallocInternalMemalign(layout.align(), layout.size())
        } else {
            bindings::TCMallocInternalMalloc(layout.size())
        };
        ptr as *mut u8
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() == 0 {
            // Pointer was from zero_sized_dangling_ptr, no tcmalloc call.
            debug_assert!(ptr == zero_sized_dangling_ptr(layout.align()));
            return;
        }

        let usable_size = bindings::TCMallocInternalMallocSize(ptr as *mut c_void);
        bindings::TCMallocInternalFreeSized(ptr as *mut c_void, usable_size);
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return zero_sized_dangling_ptr(layout.align());
        }

        let ptr = if needs_custom_alignment(layout.align(), layout.size()) {
            let p = bindings::TCMallocInternalMemalign(layout.align(), layout.size()) as *mut u8;
            if !p.is_null() {
                ptr::write_bytes(p, 0, layout.size());
            }
            p
        } else {
            bindings::TCMallocInternalCalloc(1, layout.size()) as *mut u8
        };
        ptr
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.size() == 0 {
            debug_assert!(ptr == zero_sized_dangling_ptr(layout.align()));
            return self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
        }

        if new_size == 0 {
            self.dealloc(ptr, layout);
            return zero_sized_dangling_ptr(layout.align());
        }

        // For realloc, if alignment requirements are strict, the alloc-copy-free path is safer.
        // TCMallocInternalRealloc might not preserve alignment beyond its default.
        if layout.align() > TCMALLOC_DEFAULT_ALIGN {
            let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
            let new_ptr = self.alloc(new_layout);
            if !new_ptr.is_null() {
                let copy_size = cmp::min(layout.size(), new_size);
                ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
                self.dealloc(ptr, layout); // dealloc will use the robust method
            }
            new_ptr
        } else {
            // Standard realloc.
            bindings::TCMallocInternalRealloc(ptr as *mut c_void, new_size) as *mut u8
        }
    }
}
