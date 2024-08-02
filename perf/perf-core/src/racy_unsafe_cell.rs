use std::cell::UnsafeCell;

#[repr(transparent)]
pub struct RacyUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T> Sync for RacyUnsafeCell<T> {}

impl<T> RacyUnsafeCell<T> {
    pub const fn new(x: T) -> Self {
        RacyUnsafeCell(UnsafeCell::new(x))
    }

    pub fn get(&self) -> *mut T {
        self.0.get()
    }
}
