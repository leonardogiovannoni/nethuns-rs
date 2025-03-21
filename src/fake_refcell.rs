use std::cell::UnsafeCell;

#[derive(Debug)]
pub struct FakeRefCell<T> {
    value: UnsafeCell<T>,
}

impl<T: Clone> Clone for FakeRefCell<T> {
    fn clone(&self) -> Self {
        Self {
            value: UnsafeCell::new(unsafe { (*self.value.get()).clone() }),
        }
    }
}

impl<T> FakeRefCell<T> {
    /// Creates a new FakeRefCell containing `value`.
    pub fn new(value: T) -> Self {
        FakeRefCell {
            value: UnsafeCell::new(value),
        }
    }

    /// Returns an immutable reference to the contained value.
    ///
    /// # Safety
    /// This method does not enforce Rust’s borrowing rules and is
    /// completely unchecked.
    pub fn borrow(&self) -> &T {
        unsafe { &*self.value.get() }
    }

    /// Returns a mutable reference to the contained value.
    ///
    /// # Safety
    /// This method does not enforce Rust’s borrowing rules and is
    /// completely unchecked.
    pub fn borrow_mut(&self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }
}

unsafe impl<T: Send> Send for FakeRefCell<T> {}
