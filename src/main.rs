use std::sync::atomic::{AtomicU32, AtomicBool, AtomicPtr, Ordering::{Relaxed, Release, Acquire, AcqRel, SeqCst}};
use std::ops::{Deref, DerefMut};

/// A an implementation of a "read, copy, update" data structure that uses
/// reference counting for managing de-allocation.
pub struct Rcu<T> {
    /// Holds the data `T`
    data_ptr: AtomicPtr<T>,
    /// Holds a pointer to the previous data, used for updating
    prev_ptr: AtomicPtr<T>,
    /// Holds the count of the current number of readers
    cur_readers: AtomicU32,
    /// Flag denotes whether a thread is currently writing to the data, prevents writer starvation
    write_flag: AtomicBool,

}

impl<T> Rcu<T> {
    /// Associated method for creating a new `Rcu`.
    pub fn new(value: T) -> Self {
        let data_ptr = Box::into_raw(Box::new(value));
        Self {
            data_ptr: AtomicPtr::new(data_ptr),
            prev_ptr: AtomicPtr::new(data_ptr),
            cur_readers: AtomicU32::new(0),
            write_flag: AtomicBool::new(false),
        }
    }
    /// Method that will attempt to update the data held by the `Rcu`. Returns a boolean,
    /// true if the update was successful, false otherwise.
    pub fn update(&self, new_val: T) -> bool {
        let neo = Box::into_raw(Box::new(new_val));
        let prev = self.prev_ptr.load(Relaxed);
        if let Ok(old) = self.data_ptr.compare_exchange(prev, neo, SeqCst, Relaxed) {
            // Success case, from this point on we know no other threads will
            // read the data that was previously held in `self.data_ptr`
            // Therefore, once `self.cur_readers` is 0, we can deallocate old,
            // since any thread that was reading from old has finished reading
            // Set `self.write_flag` to true to pause readers from preventing the update to `self.prev_ptr`
            self.write_flag.store(true, SeqCst);
            while self.cur_readers.load(SeqCst) > 0 {
                std::hint::spin_loop();
            }
            // Drop old i.e. data that was held in `self.prev_ptr`
            unsafe {
                drop(Box::from_raw(old));
            }
            // Reset `self.prev_ptr` to newly allocated data
            self.prev_ptr.store(neo, SeqCst);
            self.write_flag.store(false, SeqCst);
            true
        } else {
            // Unsuccessful, just drop neo
            unsafe {
                drop(Box::from_raw(neo));
            }
            false
        }
    }
}
impl<T: Clone> Rcu<T> {
    /// Reads the data currently held in `self.data_ptr`.
    pub fn read(&self) -> T {
        // Check if a thread is currently in the process of writing
        while self.write_flag.load(SeqCst) {
            std::hint::spin_loop();
        }
        self.cur_readers.fetch_add(1, SeqCst);
        // Safety: `self.data_ptr` will never be null
        let value = unsafe { (*self.data_ptr.load(SeqCst)).clone() };
        self.cur_readers.fetch_sub(1, SeqCst);
        value
    }
}
//
// pub struct RcuReadGuard<T> {
//     /// The pointer to the the `T` currently held in the `Rcu`
//     ptr: *mut T,
//     /// Holds the count of total number of threads holding this pointer
//     ref_count: *mut AtomicU32,
// }
//
// impl<T> Deref for RcuReadGuard<T> {
//     type Target = T;
//     fn deref(&self) -> &Self::Target {
//         // Safety: we are granting read only access to the current `T`
//         // and we know `self.ptr` is never null.
//         unsafe { &*self }
//     }
// }
//
// impl<T> Drop for RcuReadGuard<T> {
//     fn drop(&mut self) {
//         // Check if this is the last guard that holds a pointer to `T` and a pointer to
//         // the `AtomicU32` that represents the reference count of `T`
//         // Safety: we know `self.ptr` and `self.ref_count` are never both non null
//         unsafe {
//             if (*self.ref_count).fetch_sub(1, AcqRel) == 1 {
//                 drop(Box::from_raw(self.ptr));
//                 drop(Box::from_raw(self.ref_count));
//             }
//         }
//
//     }
// }




fn main() {
    println!("Hello, world!");
}
