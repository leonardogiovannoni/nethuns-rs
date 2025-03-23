use std::cell::RefCell;

use crate::api::{
    BufferConsumer, BufferIndex, BufferProducer, Strategy, StrategyArgs,
};

#[derive(Clone)]
pub struct MpscStrategy;

impl Strategy for MpscStrategy {
    type Producer = MpscProducer;
    type Consumer = MpscConsumer;

    fn create(nbufs: usize, args: StrategyArgs) -> (Self::Producer, Self::Consumer) {
        let args = match args {
            StrategyArgs::Mpsc(args) => args,
            _ => panic!("Invalid argument type"),
        };
        let (producer, consumer) = mpsc::channel(
            nbufs,
            args.consumer_buffer_size,
            args.producer_buffer_size,
        );
        (
            MpscProducer { inner: producer },
            MpscConsumer { inner: consumer },
        )
    }
}

#[derive(Clone, Debug)]
pub struct MpscArgs {
    pub consumer_buffer_size: usize,
    pub producer_buffer_size: usize,
}

const DEFAULT_BUFFER_SIZE: usize = 65536;
const DEFAULT_CONSUMER_BUFFER_SIZE: usize = 256;
const DEFAULT_PRODUCER_BUFFER_SIZE: usize = 256;

impl Default for MpscArgs {
    fn default() -> Self {
        Self {
            consumer_buffer_size: DEFAULT_CONSUMER_BUFFER_SIZE,
            producer_buffer_size: DEFAULT_PRODUCER_BUFFER_SIZE,
        }
    }
}


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

// STD

#[derive(Clone)]
pub struct StdStrategy;

impl Strategy for StdStrategy {
    type Producer = StdProducer;
    type Consumer = StdConsumer;

    fn create(nbufs: usize, args: StrategyArgs) -> (Self::Producer, Self::Consumer) {
        let args = match args {
            StrategyArgs::Std(args) => args,
            _ => panic!("Invalid argument type"),
        };
        let (producer, consumer) = std::sync::mpsc::channel();
        (
            StdProducer {
                inner: RefCell::new(producer),
            },
            StdConsumer {
                inner: RefCell::new(consumer),
            },
        )
    }
}

#[derive(Clone, Debug)]
pub struct StdArgs;

impl Default for StdArgs {
    fn default() -> Self {
        Self {}
    }
}


pub struct StdConsumer {
    inner: RefCell<std::sync::mpsc::Receiver<BufferIndex>>,
}

impl BufferConsumer for StdConsumer {
    fn pop(&mut self) -> Option<BufferIndex> {
        self.inner.borrow_mut().recv().ok()
    }

    fn available_len(&self) -> usize {
        todo!()
    }

    fn sync(&mut self) {
        // do nothing
    }
}

#[derive(Clone)]
pub struct StdProducer {
    inner: RefCell<std::sync::mpsc::Sender<BufferIndex>>,
}

impl BufferProducer for StdProducer {
    fn push(&mut self, elem: BufferIndex) {
        self.inner.borrow_mut().send(elem).unwrap();
    }

    fn flush(&mut self) {
        // do nothing
    }
}

// CROSSBEAM

#[derive(Clone)]
pub struct CrossbeamStrategy;

impl Strategy for CrossbeamStrategy {
    type Producer = CrossbeamProducer;
    type Consumer = CrossbeamConsumer;

    fn create(nbufs: usize, args: StrategyArgs) -> (Self::Producer, Self::Consumer) {
        let args = match args {
            StrategyArgs::Crossbeam(args) => args,
            _ => panic!("Invalid argument type"),
        };
        let (producer, consumer) = crossbeam_channel::bounded(args.buffer_size);
        (
            CrossbeamProducer { inner: producer },
            CrossbeamConsumer { inner: consumer },
        )
    }
}

#[derive(Clone, Debug)]
pub struct CrossbeamArgs {
    pub buffer_size: usize,
}

impl Default for CrossbeamArgs {
    fn default() -> Self {
        Self {
            buffer_size: DEFAULT_BUFFER_SIZE,
        }
    }
}


pub struct CrossbeamConsumer {
    inner: crossbeam_channel::Receiver<BufferIndex>,
}

impl BufferConsumer for CrossbeamConsumer {
    fn pop(&mut self) -> Option<BufferIndex> {
        self.inner.recv().ok()
    }

    fn available_len(&self) -> usize {
        todo!()
    }

    fn sync(&mut self) {
        // do nothing
    }
}

#[derive(Clone)]
pub struct CrossbeamProducer {
    inner: crossbeam_channel::Sender<BufferIndex>,
}

impl BufferProducer for CrossbeamProducer {
    fn push(&mut self, elem: BufferIndex) {
        self.inner.send(elem).unwrap();
    }

    fn flush(&mut self) {
        // do nothing
    }
}
