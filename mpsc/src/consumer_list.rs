use std::sync::Arc;

use parking_lot::Mutex;

use crate::spsc;

pub(crate) struct ConsumerList<T> {
    list: Arc<Mutex<Vec<spsc::Consumer<T>>>>,
    pub(crate) queue_len: usize,
}

impl<T> Clone for ConsumerList<T> {
    fn clone(&self) -> Self {
        Self {
            list: self.list.clone(),
            queue_len: self.queue_len.clone(),
        }
    }
}

impl<T> ConsumerList<T> {
    pub(crate) fn new(queue_len: usize) -> Self {
        Self {
            list: Arc::new(Mutex::new(Vec::new())),
            queue_len,
        }
    }

    pub(crate) fn push(&self, consumer: spsc::Consumer<T>) {
        self.list.lock().push(consumer);
    }

    pub(crate) fn remove(&self, id: usize) {
        let mut list = self.list.lock();
        let len = list.len();
        list.retain(|x| x.id() != id);
        assert!(list.len() == len - 1);
    }

    pub(crate) fn for_each(&self, mut callback: impl FnMut(&spsc::Consumer<T>)) {
        let tmp = self.list.lock();
        for value in tmp.iter() {
            callback(value);
        }
    }

    #[inline(never)]
    #[cold]
    pub(crate) fn pop_all(&mut self, v: &mut Vec<T>) {
        self.for_each(|consumer| unsafe {
            consumer.dequeue_many(v);
        });
    }
}
