#![feature(portable_simd)]
mod spsc;
mod consumer_list;

use arrayvec::ArrayVec;
use consumer_list::{pop_all, ConsumerList};
use std::usize;

use std::simd::Simd;
#[inline]
#[cold]
fn cold() {}
#[inline]
pub fn likely(b: bool) -> bool {
    if !b {
        cold()
    }
    b
}
#[inline]
pub fn unlikely(b: bool) -> bool {
    if b {
        cold()
    }
    b
}


pub struct Consumer<T> {
    consumer: ConsumerList<Simd<usize, 16>>,
    pub cached: ArrayVec<usize, 1024>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> Consumer<T> {
    pub fn pop(&mut self) -> Option<usize> {
        if unlikely(self.cached.is_empty()) {
            self.sync();
        }
        self.cached.pop()
    }

    pub fn available_len(&self) -> usize {
        self.cached.len()
    }

    pub fn sync(&mut self) {
        //self.consumer.pop_all(&mut self.cached);
        pop_all(&mut self.consumer, &mut self.cached);
    }
}

pub struct Producer<T> {
    elem: spsc::Producer<Simd<usize, 16>>,
    list: ConsumerList<Simd<usize, 16>>,
    buffer: ArrayVec<usize, 16>,
    _marker: std::marker::PhantomData<T>,
}

fn to_simd<I: Iterator<Item = usize>>(mut iter: I) -> impl Iterator<Item = Simd<usize, 16>> {
    std::iter::from_fn(move || {
        // Create an array to hold four usize values
        let mut values = [0; 16];
        for slot in values.iter_mut() {
            // Try to get the next value; if none is available, then return None to end the iterator
            if let Some(val) = iter.next() {
                *slot = val;
            } else {
                return None;
            }
        }
        // Create a SIMD vector from the array.
        Some(Simd::from_array(values))
    })
}

impl<T> Producer<T> {
    fn new(elem: spsc::Producer<Simd<usize, 16>>, list: ConsumerList<Simd<usize, 16>>) -> Self {
        Self {
            elem,
            buffer: arrayvec::ArrayVec::new(),
            list,
            _marker: std::marker::PhantomData,
        }
    }

    #[inline(always)]
    pub fn push(&mut self, elem: impl Into<usize>) {
        // SAFETY: the only way to push an element is through this function
        // and we are sure that the buffer is not full, since at the previous
        // call we checked if the buffer was full and flushed it
        let elem = elem.into();
        unsafe { self.buffer.push_unchecked(elem) };
        if self.buffer.len() == self.buffer.capacity() {    
            self.flush();
        }
    }

    #[inline(never)]
    pub fn flush(&mut self) {
        let len = self.buffer.len();
        let iter = self.buffer.drain(..);
        let iter = to_simd(iter);
        let res = self.elem.enqueue_many(iter);
        // debug_assert_eq!(res * 16, len);
    }
}

impl<T> Clone for Producer<T> {
    fn clone(&self) -> Self {
        let (p, c) = spsc::channel(self.list.queue_len);
        let list = self.list.clone();
        list.push(c);
        Self::new(p, list)
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        self.flush();
        self.list.remove(self.elem.id());
    }
}

pub fn channel<T>(size: usize) -> (Producer<T>, Consumer<T>) {
    let list = ConsumerList::new(size);
    let (p, c) = spsc::channel(size);
    list.push(c);
    (
        Producer::new(
            p,
            list.clone(),

        ),
        Consumer {
            consumer: list,
            cached: ArrayVec::new(),
            _marker: std::marker::PhantomData,
        },
    )
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() {
        const LEN: usize = 1024 * 1024 * 4;
        let (producer, mut consumer) = channel(LEN);
        let threads = num_cpus::get();
        let mut handles = Vec::new();
        let mut producers = Vec::new();
        for _ in 0..threads {
            producers.push(producer.clone());
        }
        for mut producer in producers {
            let handle = std::thread::spawn(move || {
                for i in 0..LEN {
                    producer.push(i as usize);
                }
            });
            handles.push(handle);
        }

        for i in 0..LEN {
            if let Some(val) = consumer.pop() {
        
            }
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }



}

