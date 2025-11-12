// SPMC

use ringbuf::storage::{Heap, Storage};
use ringbuf::{traits::Split, HeapRb};
use ringbuf::rb::shared::SharedRb;

pub struct SpscChannel<T> {
    prod: <SharedRb<Heap<T>> as Split>::Prod,
    cons: <SharedRb<Heap<T>> as Split>::Cons,
}

fn pippo() {
    let tmp = HeapRb::<i32>::new(2);
    let (prod, cons) = tmp.split();
}