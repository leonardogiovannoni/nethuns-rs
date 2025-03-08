mod spsc;
mod consumer_list;

use consumer_list::ConsumerList;
use std::usize;


pub struct Consumer<T> {
    consumer: ConsumerList<T>,
    cached: Vec<T>,
}

impl<T> Consumer<T> {
    pub fn pop(&mut self) -> Option<T> {
        if self.cached.is_empty() {
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



const BUFFER_LEN: usize = 256;
pub struct Producer<T> {
    elem: spsc::Producer<T>,
    buffer: arrayvec::ArrayVec<T, BUFFER_LEN>,
    list: ConsumerList<T>,
}

impl<T> Producer<T> {

    fn new(elem: spsc::Producer<T>, list: ConsumerList<T>) -> Self {
        Self {
            elem,
            buffer: arrayvec::ArrayVec::new(),
            // Vec::with_capacity(BUFFER_LEN),
            list,
        }
    }

    pub fn push(&mut self, elem: T) {
        // self.elem.enqueue(elem);
        self.buffer.push(elem);
        if self.buffer.len() == BUFFER_LEN {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        let len = self.buffer.len();
        let res = self.elem.enqueue_many(self.buffer.drain(..));
        assert_eq!(res, len);
    }
}

impl<T> Clone for Producer<T> {
    fn clone(&self) -> Self {
        let (p, c) = spsc::Queue::new(self.list.queue_len).split();
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
    let (p, c) = spsc::Queue::new(size).split();
    list.push(c);
    (
        Producer::new(
            p,
            list.clone(),
        ),
        Consumer {
            consumer: list,
            cached: Vec::with_capacity(size),
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
                    producer.push(i as u32);
                }
            });
            handles.push(handle);
        }

        //consumer.pop();

        for i in 0..LEN {
            if let Some(val) = consumer.pop() {
        
            }
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
