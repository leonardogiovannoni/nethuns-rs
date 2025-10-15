/*use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use crossbeam::{queue::ArrayQueue, utils::CachePadded};

#[derive(Debug, Clone, Copy)]
pub(crate) struct RawIndex {
    index: usize,
}

#[derive(Debug)]
struct RawBufferPoll<T> {
    //buffer: *mut u8,
    // size: usize,
    // slot_size: usize,
    free_list: ArrayQueue<RawIndex>,
    slots: Box<[CachePadded<Option<T>>]>,
}

impl<T> RawBufferPoll<T> {
    fn new(nslot: usize) -> Self {
        // let size = nslot * slot_size;
        // if size == 0 {
        //     panic!("Buffer size must be greater than 0");
        // }
        //let slot_size = std::mem::size_of::<CachePadded<Option<T>>>();

        // let layout = std::alloc::Layout::from_size_align(size, 1).unwrap();
        // SAFETY: layout's size is greater than 0
        // let buffer = unsafe { std::alloc::alloc(layout) };
        let mut slots = Vec::with_capacity(nslot);
        for _ in 0..nslot {
            let slot = CachePadded::new(None);
            slots.push(slot);
        }

        let free_list = ArrayQueue::new(nslot);
        for i in 0..nslot {
            free_list.push(RawIndex { index: i }).unwrap();
        }

        RawBufferPoll {
            free_list,
            slots: slots.into_boxed_slice(),
        }
    }

    unsafe fn alloc(&self) -> Option<RawIndex> {
        self.free_list.pop()
    }

    unsafe fn dealloc(&self, index: RawIndex) {
        self.free_list.push(index).unwrap();
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BufferPool<T> {
    inner: Arc<UnsafeCell<RawBufferPoll<T>>>,
    _invariant: std::marker::PhantomData<fn(T) -> T>,
}

impl<T> BufferPool<T> {
    pub(crate) fn new(nslot: usize) -> Self {
        let buffer = Arc::new(UnsafeCell::new(RawBufferPoll::new(nslot)));
        BufferPool {
            inner: buffer,
            _invariant: std::marker::PhantomData,
        }
    }

    unsafe fn get_slot(&self, index: RawIndex) -> &mut Option<T> {
        let pool = self.inner.get();
        let slots = unsafe { &raw mut (*pool).slots };
        unsafe { &mut *(*slots)[index.index] }
    }

    pub(crate) fn alloc(&self) -> Option<Entry<'_, T>> {
        let buffer: *const RawBufferPoll<T> = self.inner.get();
        let index = unsafe { (*buffer).alloc() };
        index.map(|index| Entry { index, pool: self })
    }

    pub(crate) fn len(&self) -> usize {
        let buffer: *const RawBufferPoll<T> = self.inner.get();
        unsafe { (*buffer).slots.len() }
    }
}

pub(crate) struct Entry<'pool, T> {
    index: RawIndex,
    pool: &'pool BufferPool<T>,
}

impl<'pool, T> Deref for Entry<'pool, T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        unsafe { self.pool.get_slot(self.index) }
    }
}

impl<'pool, T> DerefMut for Entry<'pool, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.pool.get_slot(self.index) }
    }
}

impl<'pool, T> Drop for Entry<'pool, T> {
    fn drop(&mut self) {
        unsafe {
            (*self.pool.inner.get()).dealloc(self.index);
        }
    }
}

// fn main() {
//     let pool = BufferPool::new(10);
//     let mut buffer = pool.alloc().unwrap();
//     *buffer = Some(10);
//     println!("{:?}", *buffer);
// }
*/

use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use crossbeam::queue::ArrayQueue;

#[derive(Debug, Clone, Copy)]
struct RawIndex {
    index: usize,
}

#[derive(Debug)]
pub(crate) struct RawBufferPoll<T> {
    free_list: ArrayQueue<RawIndex>,
    slots: Box<[Option<T>]>,
}

impl<T> RawBufferPoll<T> {
    fn new(nslot: usize) -> Self {
        let mut slots = Vec::with_capacity(nslot);
        for _ in 0..nslot {
            slots.push(None);
        }

        let free_list = ArrayQueue::new(nslot);
        for i in 0..nslot {
            free_list.push(RawIndex { index: i }).unwrap();
        }

        RawBufferPoll {
            free_list,
            slots: slots.into_boxed_slice(),
        }
    }

    unsafe fn alloc(&self) -> Option<RawIndex> {
        self.free_list.pop()
    }

    unsafe fn dealloc(&self, index: RawIndex) {
        self.free_list.push(index).unwrap();
    }
}

unsafe fn get_slot<T>(
    inner: &UnsafeCell<RawBufferPoll<T>>,
    index: RawIndex,
) -> &mut Option<T> {
    let pool = inner.get();
    let slots = unsafe { &raw mut (*pool).slots };
    unsafe { &mut (*slots)[index.index] }
}

#[derive(Debug, Clone)]
pub(crate) struct BufferPool<T> {
    inner: Arc<UnsafeCell<RawBufferPoll<T>>>,
    _invariant: std::marker::PhantomData<fn(T) -> T>,
}

impl<T> BufferPool<T> {
    pub(crate) fn new(nslot: usize) -> Self {
        let buffer = Arc::new(UnsafeCell::new(RawBufferPoll::new(nslot)));
        BufferPool {
            inner: buffer,
            _invariant: std::marker::PhantomData,
        }
    }

    pub(crate) fn alloc(&self) -> Option<Buffer<T>> {
        let buffer: *const RawBufferPoll<T> = self.inner.get();
        let index = unsafe { (*buffer).alloc() };
        index.map(|index| Buffer {
            index,
            pool: self.inner.clone(),
        })
    }

    pub(crate) fn len(&self) -> usize {
        let buffer: *const RawBufferPoll<T> = self.inner.get();
        unsafe { (*buffer).slots.len() }
    }
}

pub(crate) struct Buffer<T> {
    index: RawIndex,
    pool: Arc<UnsafeCell<RawBufferPoll<T>>>,
}

impl<T> Deref for Buffer<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        // unsafe { (*self.pool.get()).get_slot(self.index) }
        unsafe { get_slot(&self.pool, self.index) }
    }
}

impl<T> DerefMut for Buffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // unsafe { self.pool.get_slot(self.index) }
        unsafe { get_slot(&self.pool, self.index) }
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        unsafe {
            (*self.pool.get()).dealloc(self.index);
        }
    }
}

pub(crate) struct StaticBufferPool<T: 'static> {
    slots: &'static [UnsafeCell<Option<T>>],
    free_list: ArrayQueue<usize>,
}

impl<T: 'static> StaticBufferPool<T> {
    fn new(slots: &'static [UnsafeCell<Option<T>>]) -> Self {
        let nslot = slots.len();

        let free_list = ArrayQueue::new(nslot);
        for i in 0..nslot {
            free_list.push(i).unwrap();
        }

        StaticBufferPool { slots, free_list }
    }

    fn alloc(&'static self) -> Option<StaticBuffer<T>> {
        let index = self.free_list.pop()?;
        Some(StaticBuffer { index, pool: self })
    }

    fn dealloc(&self, index: usize) {
        self.free_list.push(index).unwrap();
    }
}

pub(crate) struct StaticBuffer<T: 'static> {
    index: usize,
    pool: &'static StaticBufferPool<T>,
}

impl<T: 'static> Deref for StaticBuffer<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.pool.slots[self.index].get() }
    }
}

impl<T: 'static> DerefMut for StaticBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.pool.slots[self.index].get() }
    }
}

impl<T: 'static> Drop for StaticBuffer<T> {
    fn drop(&mut self) {
        self.pool.dealloc(self.index);
    }
}

pub unsafe trait IoBuf: Unpin + 'static {
    /// Returns a raw pointer to the vector’s buffer.
    ///
    /// This method is to be used by the `tokio-uring` runtime and it is not
    /// expected for users to call it directly.
    ///
    /// The implementation must ensure that, while the `tokio-uring` runtime
    /// owns the value, the pointer returned by `stable_ptr` **does not**
    /// change.
    fn stable_ptr(&self) -> *const u8;

    /// Number of initialized bytes.
    ///
    /// This method is to be used by the `tokio-uring` runtime and it is not
    /// expected for users to call it directly.
    ///
    /// For `Vec`, this is identical to `len()`.
    fn length(&self) -> usize;


    fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.stable_ptr(), self.length()) }
    }
}

pub unsafe trait IoBufMut: IoBuf {
    /// Returns a raw mutable pointer to the vector’s buffer.
    ///
    /// This method is to be used by the `tokio-uring` runtime and it is not
    /// expected for users to call it directly.
    ///
    /// The implementation must ensure that, while the `tokio-uring` runtime
    /// owns the value, the pointer returned by `stable_mut_ptr` **does not**
    /// change.
    fn stable_mut_ptr(&mut self) -> *mut u8;

    /// Updates the number of initialized bytes.
    ///
    /// If the specified `pos` is greater than the value returned by
    /// [`IoBuf::bytes_init`], it becomes the new water mark as returned by
    /// `IoBuf::bytes_init`.
    ///
    /// # Safety
    ///
    /// The caller must ensure that all bytes starting at `stable_mut_ptr()` up
    /// to `pos` are initialized and owned by the buffer.
    unsafe fn set_init(&mut self, pos: usize);

    fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.stable_mut_ptr(), self.length()) }
    }
}

unsafe impl IoBuf for Vec<u8> {
    fn stable_ptr(&self) -> *const u8 {
        self.as_ptr()
    }

    fn length(&self) -> usize {
        self.len()
    }
}

unsafe impl IoBufMut for Vec<u8> {
    fn stable_mut_ptr(&mut self) -> *mut u8 {
        self.as_mut_ptr()
    }

    unsafe fn set_init(&mut self, init_len: usize) {
        if self.len() < init_len {
            unsafe {
                self.set_len(init_len);
            }
        }
    }
}

unsafe impl<const N: usize> IoBuf for StaticBuffer<[u8; N]> {
    fn stable_ptr(&self) -> *const u8 {
        self.deref().as_ref().unwrap().as_ptr()
    }

    fn length(&self) -> usize {
        N
    }
}

unsafe impl<const N: usize> IoBufMut for StaticBuffer<[u8; N]> {
    fn stable_mut_ptr(&mut self) -> *mut u8 {
        self.deref_mut().as_mut().unwrap().as_mut_ptr()
    }

    unsafe fn set_init(&mut self, _init_len: usize) {}
}

fn get_static_slots<const SLOT_SIZE: usize>(
    nslots: usize,
) -> &'static [UnsafeCell<Option<[u8; SLOT_SIZE]>>] {
    let mut v = Vec::new();
    for _ in 0..nslots {
        v.push(UnsafeCell::new(None));
    }
    let slots: &'static _ = Box::leak(v.into_boxed_slice());
    slots
}

pub(crate) fn new_static_buffer_pool<const SLOT_SIZE: usize>(
    nslots: usize,
) -> &'static StaticBufferPool<[u8; SLOT_SIZE]> {
    let slots = get_static_slots::<SLOT_SIZE>(nslots);
    Box::leak(Box::new(StaticBufferPool::new(slots)))
}
