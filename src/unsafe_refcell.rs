use core::cell::{Cell, UnsafeCell};
use core::ops::{Deref, DerefMut};

/// A `RefCell`-like container where `borrow()` and `borrow_mut()` are *always unsafe*.
/// In debug builds, a runtime borrow counter is enforced:
///   * `>= 0` => number of shared borrows
///   * `-1`   => an exclusive (mutable) borrow is active
///
/// In release builds, the counter remains present but is not used, and no checks occur.
/// The caller must uphold all aliasing rules when calling the unsafe methods.
pub struct UnsafeRefCell<T> {
    value: UnsafeCell<T>,
    // Present in all builds. Checked/updated only in debug builds.
    borrow: Cell<isize>,
}

impl<T> UnsafeRefCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            borrow: Cell::new(0),
        }
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T> UnsafeRefCell<T> {
    /// SAFETY: The caller must ensure no mutable borrow is active and that
    /// aliasing rules are upheld for the returned shared reference.
    pub unsafe fn borrow<'a>(&'a self) -> UnsafeRef<'a, T> {
        #[cfg(debug_assertions)]
        {
            let b = self.borrow.get();
            debug_assert!(b >= 0, "UnsafeRefCell already mutably borrowed");
            self.borrow.set(b + 1);
        }

        UnsafeRef {
            value: unsafe { &*self.value.get() },
            cell: self,
        }
    }

    /// SAFETY: The caller must ensure no other borrows (shared or mutable)
    /// overlap with the returned mutable reference.
    pub unsafe fn borrow_mut<'a>(&'a self) -> UnsafeRefMut<'a, T> {
        #[cfg(debug_assertions)]
        {
            let b = self.borrow.get();
            debug_assert!(b == 0, "UnsafeRefCell already borrowed");
            self.borrow.set(-1);
        }

        UnsafeRefMut {
            value: unsafe { &mut *self.value.get() },
            cell: self,
        }
    }

    /// Get a unique mutable reference when you have `&mut self` (always safe).
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    /// Raw pointer to the inner value (useful for FFI / advanced use).
    pub fn as_ptr(&self) -> *mut T {
        self.value.get()
    }
}

/// Shared-borrow RAII guard (like `std::cell::Ref`), used only to maintain
/// debug borrow counts. In release builds it’s effectively zero-cost.
pub struct UnsafeRef<'a, T> {
    value: &'a T,
    cell: &'a UnsafeRefCell<T>,
}

impl<'a, T> Deref for UnsafeRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T> Drop for UnsafeRef<'a, T> {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            let b = self.cell.borrow.get();
            debug_assert!(b > 0, "UnsafeRefCell borrow counter underflow");
            self.cell.borrow.set(b - 1);
        }
    }
}

/// Unique-borrow RAII guard (like `std::cell::RefMut`), used only to maintain
/// debug borrow counts. In release builds it’s effectively zero-cost.
pub struct UnsafeRefMut<'a, T> {
    value: &'a mut T,
    cell: &'a UnsafeRefCell<T>,
}

impl<'a, T> Deref for UnsafeRefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T> DerefMut for UnsafeRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<'a, T> Drop for UnsafeRefMut<'a, T> {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            let b = self.cell.borrow.get();
            debug_assert!(b == -1, "UnsafeRefCell borrow counter corrupted");
            self.cell.borrow.set(0);
        }
    }
}
/*
impl<T: Clone> Clone for FakeRefCell<T> {
    fn clone(&self) -> Self {
        Self {
            value: UnsafeCell::new(unsafe { (*self.value.get()).clone() }),
        }
    }
} */

impl<T: Clone> Clone for UnsafeRefCell<T> {
    fn clone(&self) -> Self {
        Self {
            value: UnsafeCell::new(unsafe { (*self.value.get()).clone() }),
            borrow: Cell::new(0),
        }
    }
}

unsafe impl<T: Send> Send for UnsafeRefCell<T> {}
