use std::cell::RefCell;

use crate::api::bdistributor::{BDistributor, PopError, PushError, TryPopError, TryPushError};
use crate::api::bdistributor::nspscbdistributor::{
    NSPSCBDistributor, NSPSCBDistributorPopper, NSPSCBDistributorPusher,
};

pub struct MultiSender<T> {
    senders: RefCell<Vec<aspsc::Sender<T>>>,
}

impl<T> MultiSender<T> {
    fn assert_index(&self, index: usize) {
        let len = self.senders.borrow().len();
        assert!(index < len, "index out of bounds: {index} >= {len}");
    }
}

pub struct Receiver<T> {
    receiver: RefCell<aspsc::Receiver<T>>,
}

pub fn nspsc_channel<T: Send>(capacity: usize, n: usize) -> (MultiSender<T>, Vec<Receiver<T>>) {
    let mut senders = Vec::with_capacity(n);
    let mut receivers = Vec::with_capacity(n);

    for _ in 0..n {
        let (s, r) = aspsc::bounded(capacity);
        senders.push(s);
        receivers.push(Receiver {
            receiver: RefCell::new(r),
        });
    }

    (
        MultiSender {
            senders: RefCell::new(senders),
        },
        receivers,
    )
}

impl<const BATCH_SIZE: usize, T: Send + 'static> BDistributor<BATCH_SIZE, T>
    for (MultiSender<[T; BATCH_SIZE]>, Vec<Receiver<[T; BATCH_SIZE]>>) 
{
}

impl<const BATCH_SIZE: usize, T: Send + 'static> NSPSCBDistributor<BATCH_SIZE, T>
    for (MultiSender<[T; BATCH_SIZE]>, Vec<Receiver<[T; BATCH_SIZE]>>)
{
    type Pusher = MultiSender<[T; BATCH_SIZE]>;
    type Popper = Receiver<[T; BATCH_SIZE]>;

    fn split(self) -> (Self::Pusher, Vec<Self::Popper>) {
        // nspsc_channel::<[T; BATCH_SIZE]>(1024, n)
        let (pusher, poppers) = self; 
        (pusher, poppers)
    }
}

impl<const BATCH_SIZE: usize, T: Send + 'static> NSPSCBDistributorPusher<BATCH_SIZE, T>
    for MultiSender<[T; BATCH_SIZE]>
{
    fn try_push(
        &self,
        batch: [T; BATCH_SIZE],
        index: usize,
    ) -> core::result::Result<(), TryPushError<[T; BATCH_SIZE]>> {
        self.assert_index(index);
        let mut senders = self.senders.borrow_mut();
        senders[index].try_send(batch).map_err(|err| match err {
            aspsc::TrySendError::Full(batch) => TryPushError::Full(batch),
            aspsc::TrySendError::Closed(batch) => TryPushError::Closed(batch),
        })
    }

    fn push(
        &self,
        batch: [T; BATCH_SIZE],
        index: usize,
    ) -> impl core::future::Future<Output = core::result::Result<(), PushError<[T; BATCH_SIZE]>>> {
        self.assert_index(index);
        async move {
            let mut senders = self.senders.borrow_mut();
            senders[index]
                .send(batch)
                .await
                .map_err(|err| PushError(err.0))
        }
    }
}

impl<const BATCH_SIZE: usize, T: Send + 'static> NSPSCBDistributorPopper<BATCH_SIZE, T>
    for Receiver<[T; BATCH_SIZE]>
{
    fn try_pop(&self) -> Result<[T; BATCH_SIZE], TryPopError> {
        let mut receiver = self.receiver.borrow_mut();
        receiver.try_recv().map_err(|err| match err {
            aspsc::TryRecvError::Empty => TryPopError::Empty,
            aspsc::TryRecvError::Closed => TryPopError::Closed,
        })
    }

    fn pop(
        &self,
    ) -> impl core::future::Future<Output = Result<[T; BATCH_SIZE], PopError>> {
        async move {
            let mut receiver = self.receiver.borrow_mut();
            receiver.recv().await.map_err(|_| PopError)
        }
    }
}
