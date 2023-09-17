use std::sync::atomic::{AtomicU32, AtomicPtr, Ordering::{Relaxed, Release, Acquire, AcqRel}};
use std::ops::{Deref, DerefMut};

/// A an implementation of a "read, copy, update" data structure that uses
/// reference counting for managing de-allocation.
pub struct Rcu<T> {
    /// Holds the data `T`
    ptr: AtomicPtr<T>,
    /// Holds the current reference count of `T`
    ref_count: AtomicPtr<AtomicU32>,
}

impl<T> Rcu<T> {
    /// Associated method for creating a new `Rcu`.
    pub fn new(value: T) -> Self {
        Self {
            ptr: AtomicPtr::new(Box::into_raw(Box::new(value))),
            ref_count: AtomicPtr::new(Box::into_raw(Box::new(AtomicU32::new(0)))),
        }
    }
    /// Returns a `RcuReadGuard` to the currently held data
    pub fn read(&self) -> RcuReadGuard<T> {
        // Safety: We know `self.ref_count` is never null.
        unsafe { (*self.ref_count.load(Acquire)).fetch_add(1, AcqRel); }
        RcuReadGuard {
            ptr: self.ptr.load(Relaxed),
            ref_count: self.ref_count.load(Relaxed),
        }
    }

    pub fn update(&self, guard: RcuReadGuard<T>, new_val: T) {
        if self.ptr.compare_exchange(
            guard.ptr,
            Box::into_raw(Box::new(new_val)),
            Acquire,
            Relaxed).is_ok() {
            self.ref_count.swap(Box::into_raw(Box::new(AtomicU32::new(0))), Release);
        }
    }


}

pub struct RcuReadGuard<T> {
    /// The pointer to the the `T` currently held in the `Rcu`
    ptr: *mut T,
    /// Holds the count of total number of threads holding this pointer
    ref_count: *mut AtomicU32,
}

impl<T> Deref for RcuReadGuard<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // Safety: we are granting read only access to the current `T`
        // and we know `self.ptr` is never null.
        unsafe { &*self }
    }
}

impl<T> Drop for RcuReadGuard<T> {
    fn drop(&mut self) {
        // Check if this is the last guard that holds a pointer to `T` and a pointer to
        // the `AtomicU32` that represents the reference count of `T`
        // Safety: we know `self.ptr` and `self.ref_count` are never both non null
        unsafe {
            if (*self.ref_count).fetch_sub(1, AcqRel) == 1 {
                drop(Box::from_raw(self.ptr));
                drop(Box::from_raw(self.ref_count));
            }
        }

    }
}




fn main() {
    println!("Hello, world!");
}
