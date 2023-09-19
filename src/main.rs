use std::sync::atomic::{AtomicU32, AtomicBool, AtomicPtr, Ordering::{Relaxed, Release, Acquire, AcqRel, SeqCst}};
use std::clone::Clone;
use std::thread;
use std::sync::Arc;
use std::ops::{Deref, DerefMut};
use rand::{self, Rng, thread_rng};

/// A an implementation of a "read, copy, update" data structure that uses
/// reference counting for managing de-allocation.
pub struct Rcu<T: Clone> {
    /// Holds the data `T`
    data_ptr: AtomicPtr<T>,
    /// Holds a pointer to the previous data, used for updating
    prev_ptr: AtomicPtr<T>,
    /// Holds the count of the current number of readers
    cur_readers: AtomicU32,
    /// Flag denotes whether a thread is currently writing to the data, prevents writer starvation
    write_flag: AtomicBool,

}

impl<T: Clone> Rcu<T> {
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
    /// Create a subscriber to the `Rcu`
    pub fn subscribe(&self) -> RcuSubscriber<T> {
        RcuSubscriber {
            data_ptr_ref: &self.data_ptr,
            cur_readers_ref: &self.cur_readers,
            write_flag_ref: &self.write_flag,
        }
    }
    /// Reads the data currently held in `self.data_ptr`. Returns a cloned version of the current T held by the `Rcu`.
    pub fn read(&self) -> T {
        // Check if a thread is currently in the process of writing
        // Acquire matches the Release from `self.update`
        while self.write_flag.load(Acquire) {
            std::hint::spin_loop();
        }
        self.cur_readers.fetch_add(1, SeqCst);
        // Safety: `self.data_ptr` will never be null
        let data = unsafe { (*self.data_ptr.load(SeqCst)).clone() };
        self.cur_readers.fetch_sub(1, SeqCst);
        data
    }
    /// Method that will attempt to update the data held by the `Rcu`. Returns a boolean,
    /// true if the update was successful, false otherwise.
    pub fn update(&self, new_val: T) -> bool {
        let neo = Box::into_raw(Box::new(new_val));
        let prev = self.prev_ptr.load(Acquire);
        // Ensure that we are not interrupting a concurrent update
        while self.write_flag.load(Acquire) {
            std::hint::spin_loop();
        }
        if let Ok(old) = self.data_ptr.compare_exchange(prev, neo, Release, Relaxed) {
            // Success case, from this point on we know no new threads will
            // read the data that was previously held in `self.data_ptr`
            // Therefore, once `self.cur_readers` is 0, we can deallocate old,
            // since any thread that was reading from old has finished reading
            // Set `self.write_flag` to true to pause readers from preventing the update to `self.prev_ptr`
            self.write_flag.store(true, Release);
            while self.cur_readers.load(SeqCst) > 0 {
                std::hint::spin_loop();
            }
            // Reset `self.prev_ptr` to newly allocated data, for future updates
            self.prev_ptr.store(neo, Release);
            // Safety: We know nothing will read from old ever again
            // so drop old, i.e. data that was held in `self.prev_ptr`
            unsafe {
                drop(Box::from_raw(old));
            }
            self.write_flag.store(false, Release);
            true
        } else {
            // Safety: We know nothing will read from neo ever again.
            // Unsuccessful, just drop neo
            unsafe {
                drop(Box::from_raw(neo));
            }
            false
        }
    }
}

unsafe impl<T> Send for Rcu<T> where T: Send + Sync + Clone {}
unsafe impl<T> Sync for Rcu<T> where T: Send + Sync + Clone {}

/// A struct for subscribing to a `Rcu`. May be useful when a thread only needs to read the current value of the
/// `Rcu` and does not need have the ability to update.
pub struct RcuSubscriber<'a, T: Clone> {
    data_ptr_ref: &'a AtomicPtr<T>,
    cur_readers_ref: &'a AtomicU32,
    write_flag_ref: &'a AtomicBool,
}

impl<T: Clone> RcuSubscriber<'_, T> {
    /// Read the data held in `self.data_ptr_ref`, the data that is currently in the `Rcu` being subscribed to.
    fn read(&self) -> T {
        // Check if there is an update occurring
        while self.write_flag_ref.load(Acquire) {
            std::hint::spin_loop();
        }
        self.cur_readers_ref.fetch_add(1, SeqCst);
        // Safety: We know that `self.data_ptr_ref` will never return a null pointer
        let data = unsafe { (*self.data_ptr_ref.load(Acquire)).clone() };
        self.cur_readers_ref.fetch_sub(1, SeqCst);
        data
    }
}

unsafe impl<T> Send for RcuSubscriber<'_, T> where T: Send + Sync + Clone {}
unsafe impl<T> Sync for RcuSubscriber<'_, T> where T: Send + Sync + Clone {}


pub fn mean(nums: &Vec<i32>) -> f32 {
    nums.iter().map(|n| *n as f32).sum::<f32>() / (nums.len() as f32)
}

fn main() {
    let rcu = &Rcu::new(vec![]);
    let mut counter = &AtomicU32::new(0);
    thread::scope(|s| {
        for i in 0..20 {
            s.spawn(move || {
                let mut rng = thread_rng();
                let mut largest_mean = f32::MIN;
                for j in 0..1000 {
                    let num = rng.gen_range(-100..=100);
                    let mut data = rcu.read();
                    data.push(num);
                    let cur_mean = mean(&data);
                    if cur_mean > largest_mean {
                        largest_mean = cur_mean;
                        // Attempt to update
                        if rcu.update(data) {
                            println!("update successful from thread {i} with mean {:0.2}", largest_mean);
                        }
                    }
                    counter.fetch_add(1, Relaxed);
                }
            });
        }
    });
    while counter.load(Relaxed) < 2000 {
        std::hint::spin_loop();
    }
}
