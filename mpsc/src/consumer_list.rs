use std::simd::{usizex16, usizex4, usizex8};

use triomphe::Arc;

use arrayvec::ArrayVec;
use parking_lot::Mutex;

use crate::spsc;

pub(crate) struct ConsumerList<T> {
    list: Arc<Mutex<ArrayVec<spsc::Consumer<T>, 4096>>>,
    pub(crate) queue_len: usize,
}

impl<T> Clone for ConsumerList<T> {
    fn clone(&self) -> Self {
        Self {
            list: self.list.clone(),
            queue_len: self.queue_len,
        }
    }
}

impl<T> ConsumerList<T> {
    pub(crate) fn new(queue_len: usize) -> Self {
        Self {
            list: Arc::new(Mutex::new(ArrayVec::new())),
            queue_len,
        }
    }

    pub(crate) fn push(&self, consumer: spsc::Consumer<T>) {
        self.list.lock().push(consumer);
    }

    pub(crate) fn remove(&mut self, id: usize) {
        let mut list = self.list.lock();
        let len = list.len();
        // We have exclusive access to the list, so we can safely remove the consumer
        unsafe { 
            list.retain(|x| x.id() != id);
        }
        assert!(list.len() == len - 1);
    }

    #[inline(always)]
    pub(crate) fn for_each(&self, mut callback: impl FnMut(&spsc::Consumer<T>)) {
        let tmp = self.list.lock();
        for value in tmp.iter() {
            callback(value);
        }
    }

}



#[inline(never)]
#[cold]
pub fn pop_all<const N: usize>(me: &mut ConsumerList<usizex16>, v: &mut ArrayVec<usize, { N }>) {
    me.for_each(|consumer| {
        let consumer = unsafe { &mut *consumer.consumer.get() };
        let remaining = (v.capacity() - v.len()) / 16;
        for scan in ringbuf::traits::Consumer::pop_iter(consumer).take(remaining) {
            unsafe {
                let len = v.len();
                let ptr = v.as_mut_ptr().add(len);
                let ptr = ptr as *mut usizex16;
                std::ptr::write(ptr, scan);
                v.set_len(len + 16);
            }
        }
    });
}