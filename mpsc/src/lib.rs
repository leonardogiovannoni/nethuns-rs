mod consumer_list;
mod spsc;

use arrayvec::ArrayVec;
use consumer_list::ConsumerList;
use std::usize;

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
    consumer: ConsumerList<T>,
    pub cached: ArrayVec<T, 1024>,
}

impl<T: Copy> Consumer<T> {
    pub fn pop(&mut self) -> Option<T> {
        if unlikely(self.cached.is_empty()) {
            self.sync();
        }
        self.cached.pop()
    }

    pub fn available_len(&self) -> usize {
        self.cached.len()
    }

    pub fn sync(&mut self) {
        self.consumer.pop_all(&mut self.cached);
    }
}

pub struct Producer<T: Copy> {
    elem: spsc::Producer<T>,
    buffer: ArrayVec<T, 32>,
    list: ConsumerList<T>,
}

impl<T: Copy> Producer<T> {
    fn new(elem: spsc::Producer<T>, list: ConsumerList<T>) -> Self {
        Self {
            elem,
            buffer: arrayvec::ArrayVec::new(),
            list,
        }
    }

    #[inline(always)]
    pub fn push(&mut self, elem: T) {
        // SAFETY: the only way to push an element is through this function
        // and we are sure that the buffer is not full, since at the previous
        // call we checked if the buffer was full and flushed it
        unsafe { self.buffer.push_unchecked(elem) };
        if self.buffer.len() == self.buffer.capacity() {
            self.flush();
        }
    }

    #[inline(never)]
    pub fn flush(&mut self) {
        let len = self.buffer.len();
        let iter = self.buffer.drain(..);
        let res = self.elem.enqueue_many(iter);
        assert_eq!(res, len);
    }
}

impl<T: Copy> Clone for Producer<T> {
    fn clone(&self) -> Self {
        let (p, c) = spsc::channel(self.list.queue_len);
        let list = self.list.clone();
        list.push(c);
        Self::new(p, list)
    }
}

impl<T: Copy> Drop for Producer<T> {
    fn drop(&mut self) {
        self.flush();
        self.list.remove(self.elem.id());
    }
}

pub fn channel<T: Copy>(size: usize) -> (Producer<T>, Consumer<T>) {
    let list = ConsumerList::new(size);
    let (p, c) = spsc::channel(size);
    list.push(c);
    (
        Producer::new(p, list.clone()),
        Consumer {
            consumer: list,
            cached: ArrayVec::new(),
        },
    )
}

fn main2() {
    let now = std::time::Instant::now();
    const LEN: usize = 1024 * 1024 * 80;
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
                producer.push(i);
            }
        });
        handles.push(handle);
    }

    for i in 0..LEN {
        if let Some(val) = consumer.pop() {}
    }

    for handle in handles {
        handle.join().unwrap();
    }
    println!("Time: {:?}", now.elapsed());
}
