// Copyright 2020 Xavier Gillard
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of
// this software and associated documentation files (the "Software"), to deal in
// the Software without restriction, including without limitation the rights to
// use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software is furnished to do so,
// subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
// FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
// COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
// IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! This module provides a dead simple low-overhead wrapper around the system
//! allocator which lets a program know its own memory consumption and peak
//! memory consumption at runtime.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// This atomic counter monitors the amount of memory (in bytes) that is
/// currently allocated for this process.
static CURRENT: AtomicUsize = AtomicUsize::new(0);
/// This atomic counter monitors the maximum amount of memory (in bytes) that
/// has been allocated for this process over the course of its life.
static PEAK: AtomicUsize = AtomicUsize::new(0);

/// This structure implements a dead simple low-overhead wrapper around the
/// system allocator. It lets a program know its own memory and peak memory
/// consumption at runtime.
///
/// # Note
/// The peak allocator is really just a shim around the system allocator. The
/// bulk of its work is delegated to the system allocator and all `PeakAlloc`
/// does is to maintain the atomic counters.
///
/// # Example
/// To make use of the PeakAllocator, all you need to do, is to declare a static
/// instance of it, and annotate it with the `#[global_allocator]` attribute.
/// Then, in your main module (or anywhere else in your code where it is deemed
/// useful), you just call methods on the static variable you declared.
///
/// ```
/// use peak_alloc::PeakAlloc;
///
/// #[global_allocator]
/// static PEAK_ALLOC: PeakAlloc = PeakAlloc;
///
/// fn main() {
///     // Do your funky stuff...
///
///     let current_mem = PEAK_ALLOC.current_usage_as_mb();
///     println!("This program currently uses {} MB of RAM.", current_mem);
///     let peak_mem = PEAK_ALLOC.peak_usage_as_gb();
///     println!("The max amount that was used {}", peak_mem);
/// }
/// ```
#[derive(Debug, Default, Copy, Clone)]
pub struct PeakAlloc;

impl PeakAlloc {
    /// Returns the number of bytes that are currently allocated to the process
    pub fn current_usage(&self) -> usize {
        CURRENT.load(Ordering::Relaxed)
    }
    /// Returns the maximum number of bytes that have been allocated to the
    /// process over the course of its life.
    pub fn peak_usage(&self) -> usize {
        PEAK.load(Ordering::Relaxed)
    }
    /// Returns the amount of memory (in kb) that is currently allocated
    /// to the process.
    pub fn current_usage_as_kb(&self) -> f32 {
        Self::kb(self.current_usage())
    }
    /// Returns the amount of memory (in mb) that is currently allocated
    /// to the process.
    pub fn current_usage_as_mb(&self) -> f32 {
        Self::mb(self.current_usage())
    }
    /// Returns the amount of memory (in gb) that is currently allocated
    /// to the process.
    pub fn current_usage_as_gb(&self) -> f32 {
        Self::gb(self.current_usage())
    }
    /// Returns the maximum quantity of memory (in kb) that have been allocated
    /// to the process over the course of its life.
    pub fn peak_usage_as_kb(&self) -> f32 {
        Self::kb(self.peak_usage())
    }
    /// Returns the maximum quantity of memory (in mb) that have been allocated
    /// to the process over the course of its life.
    pub fn peak_usage_as_mb(&self) -> f32 {
        Self::mb(self.peak_usage())
    }
    /// Returns the maximum quantity of memory (in gb) that have been allocated
    /// to the process over the course of its life.
    pub fn peak_usage_as_gb(&self) -> f32 {
        Self::gb(self.peak_usage())
    }
    /// Resets the peak usage to the value currently in memory
    pub fn reset_peak_usage(&self) {
        PEAK.store(CURRENT.load(Ordering::Relaxed), Ordering::Relaxed);
    }
    /// Performs the bytes to kilobytes conversion
    fn kb(x: usize) -> f32 {
        x as f32 / 1024.0
    }
    /// Performs the bytes to megabytes conversion
    fn mb(x: usize) -> f32 {
        x as f32 / (1024.0 * 1024.0)
    }
    /// Performs the bytes to gigabytes conversion
    fn gb(x: usize) -> f32 {
        x as f32 / (1024.0 * 1024.0 * 1024.0)
    }

    fn add_memory(&self, size: usize) {
        // as pointed out by @luxalpa, fetch_add returns the PREVIOUS value.
        let prev = CURRENT.fetch_add(size, Ordering::Relaxed);
        PEAK.fetch_max(prev + size, Ordering::Relaxed);
    }

    fn sub_memory(&self, size: usize) {
        CURRENT.fetch_sub(size, Ordering::Relaxed);
    }
}

/// PeakAlloc only implements the minimum required set of methods to make it
/// useable as a global allocator (with `#[global_allocator]` attribute).
///
/// No funky stuff is done below.
unsafe impl GlobalAlloc for PeakAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ret = System.alloc(layout);
        if !ret.is_null() {
            self.add_memory(layout.size())
        }
        ret
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        self.sub_memory(layout.size());
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();

        // SAFETY: the safety contract for `alloc` must be upheld by the caller.
        let ret = System.alloc(layout);
        if !ret.is_null() {
            self.add_memory(size);

            // SAFETY: as allocation succeeded, the region from `ptr`
            // of size `size` is guaranteed to be valid for writes.
            std::ptr::write_bytes(ret, 0, size);
        }
        ret
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let size = layout.size();

        // SAFETY: the caller must ensure that the `new_size` does not overflow.
        // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());

        // SAFETY: the caller must ensure that `new_layout` is greater than zero.
        let new_ptr = System.alloc(new_layout);
        if !new_ptr.is_null() {
            self.add_memory(new_size);

            // SAFETY: the previously allocated block cannot overlap the newly allocated block.
            // The safety contract for `dealloc` must be upheld by the caller.
            std::ptr::copy_nonoverlapping(ptr, new_ptr, std::cmp::min(size, new_size));

            System.dealloc(ptr, layout);
            self.sub_memory(size);
        }
        new_ptr
    }
}

#[cfg(test)]
mod tests {
    use crate::{CURRENT, PEAK};

    #[global_allocator]
    static PEAK_ALLOC: crate::PeakAlloc = crate::PeakAlloc;

    #[test]
    fn test_issue_4() {
        // neutralize process allocated memory etc.. (makes it easier to reason about)
        CURRENT.store(0, std::sync::atomic::Ordering::Relaxed);
        PEAK.store   (0, std::sync::atomic::Ordering::Relaxed);

        // initially both
        assert_eq!(0, PEAK_ALLOC.current_usage());
        assert_eq!(0, PEAK_ALLOC.peak_usage());

        // make one allocation:
        {
            let mut data = vec![0_u32; 1000];

            assert_eq!(4000, PEAK_ALLOC.current_usage());
            assert_eq!(4000, PEAK_ALLOC.peak_usage());     // before the fix, this would fail

            let mut tot = 0;
            for (i, x) in data.iter_mut().enumerate() {
                *x   = i as u32;
                tot += i as u32;
            }

            assert_eq!(tot, data.iter().sum::<u32>());
            // drop the allocated data
        }

        assert_eq!(0,    PEAK_ALLOC.current_usage());
        assert_eq!(4000, PEAK_ALLOC.peak_usage());
    }
}