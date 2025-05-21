use rtcmalloc::TcMalloc;
use std::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static A: TcMalloc = TcMalloc;

#[test]
fn smoke() {
    let mut a = Vec::new();
    a.reserve(1);
    a.push(3);
}

/// https://github.com/rust-lang/rust/issues/45955
#[test]
fn overaligned() {
    let size = 8;
    let align = 16; // greater than size
    let iterations = 1000;
    unsafe {
        let pointers: Vec<_> = (0..iterations)
            .map(|_| {
                let ptr = TcMalloc.alloc(Layout::from_size_align(size, align).unwrap());
                assert!(!ptr.is_null());
                ptr
            })
            .collect();
        for &ptr in &pointers {
            assert_eq!(
                (ptr as usize) % align,
                0,
                "Got a pointer less aligned than requested"
            )
        }

        // Clean up
        for &ptr in &pointers {
            TcMalloc.dealloc(ptr, Layout::from_size_align(size, align).unwrap())
        }
    }
}

#[test]
fn test_for_potential_mismatched_size_delete_scenario() {
    // This unsafe helper function simulates the problematic deallocation logic
    // where the original layout.size() is passed directly to TCMallocInternalFreeSized.
    unsafe fn deallocate_with_original_layout_size(ptr: *mut u8, layout: Layout) {
        if layout.size() == 0 {
            // If it was a zero-sized alloc from our zero_sized_dangling_ptr,
            // it wouldn't have called tcmalloc alloc, so no tcmalloc free.
            return;
        }
        // Directly call the binding, passing the original requested size.
        // This is what might cause a mismatch if tcmalloc rounded the size up.
        rtcmalloc::bindings::TCMallocInternalFreeSized(ptr as *mut std::ffi::c_void, layout.size());
        // If you were testing issues with Sdallocx, it would be:
        // rtcmalloc::bindings::TCMallocInternalSdallocx(ptr as *mut std::ffi::c_void, layout.size(), 0);
    }

    println!("Starting test_for_potential_mismatched_size_delete_scenario. If GWP-ASan is active and detects an issue, it will print to stderr.");

    // Test cases: (size, alignment)
    // Try sizes that are small or might be rounded up by tcmalloc's size classes.
    let test_layouts = [
        Layout::from_size_align(1, 8).unwrap(),  // Very small
        Layout::from_size_align(8, 8).unwrap(),  // Exact common size
        Layout::from_size_align(9, 16).unwrap(), // Just over 8, with alignment
        Layout::from_size_align(16, 16).unwrap(),
        Layout::from_size_align(17, 32).unwrap(), // Just over 16
        Layout::from_size_align(32, 32).unwrap(),
        Layout::from_size_align(33, 64).unwrap(), // Just over 32
        Layout::from_size_align(600, 64).unwrap(), // A larger size, like in your error
        Layout::from_size_align(640, 64).unwrap(), // The exact size from your error message
    ];

    let iterations_per_layout = 50; // Increase GWP-ASan's chance by repetition

    for layout in test_layouts.iter() {
        if layout.size() == 0 {
            continue;
        } // Skip ZSTs for this direct test logic

        // println!("Testing layout: size={}, align={}", layout.size(), layout.align());
        let mut allocations = Vec::with_capacity(iterations_per_layout);

        for i in 0..iterations_per_layout {
            unsafe {
                let ptr = A.alloc(*layout); // Use the proper GlobalAlloc for allocation
                if ptr.is_null() {
                    eprintln!(
                        "Allocation failed for layout {:?} at iteration {}",
                        layout, i
                    );
                    // Clean up previously successful allocations for this layout before continuing
                    for (p, l) in allocations.drain(..) {
                        deallocate_with_original_layout_size(p, l);
                    }
                    break; // Move to next layout
                }
                // Fill with a byte to ensure the memory is touched (sometimes helps tools)
                // *ptr = i as u8;
                allocations.push((ptr, *layout));
            }
        }

        // Deallocate using the potentially problematic direct call
        for (ptr, layout_stored) in allocations {
            unsafe {
                deallocate_with_original_layout_size(ptr, layout_stored);
            }
        }
    }
    println!("Finished test_for_potential_mismatched_size_delete_scenario.");
    // If GWP-ASan reported an error, you would have seen it in stderr by now.
    // The test itself will "pass" in Rust's test runner unless GWP-ASan aborts the process.
}
