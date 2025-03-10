use std::cell::RefCell;

use crate::api::{BufferConsumer, BufferIndex, BufferProducer, Strategy, StrategyArgs};



#[derive(Clone)]
pub struct MpscStrategy;

impl Strategy for MpscStrategy {
    type Producer = MpscProducer;
    type Consumer = MpscConsumer;
    type Args = MpscArgs;

    fn create(args: Self::Args) -> (Self::Producer, Self::Consumer) {
        let (producer, consumer) = mpsc::channel(args.buffer_size);
        (MpscProducer { inner: producer }, MpscConsumer { inner: consumer })
    }
}

#[derive(Clone)]
pub struct MpscArgs {
    pub buffer_size: usize,
}

const DEFAULT_BUFFER_SIZE: usize = 1024;

impl Default for MpscArgs {
    fn default() -> Self {
        Self { buffer_size: DEFAULT_BUFFER_SIZE }
    }
}

impl StrategyArgs for MpscArgs {}


pub struct MpscConsumer {
    inner: mpsc::Consumer<BufferIndex>,
}

impl BufferConsumer for MpscConsumer {
    fn pop(&mut self) -> Option<BufferIndex> {
        self.inner.pop()
    }

    fn available_len(&self) -> usize {
        self.inner.available_len()
    }

    fn sync(&mut self) {
        self.inner.sync();
    }
}

#[derive(Clone)]
pub struct MpscProducer {
    inner: mpsc::Producer<BufferIndex>,
}

impl BufferProducer for MpscProducer {
    fn push(&mut self, elem: BufferIndex) {
        self.inner.push(elem);
    }

    fn flush(&mut self) {
        self.inner.flush();
    }
}