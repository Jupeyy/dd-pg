#![allow(clippy::borrowed_box)]
use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::Arc,
};

pub trait HiarcTrait {
    const HI_VAL: u64;
}

impl HiarcTrait for u8 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for i8 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for u16 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for i16 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for u32 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for i32 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for u64 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for i64 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for u128 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for i128 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for usize {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for isize {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for f32 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for f64 {
    const HI_VAL: u64 = 0;
}

impl HiarcTrait for std::time::Duration {
    const HI_VAL: u64 = 0;
}

#[cfg(feature = "enable_parking_lot")]
impl<T> HiarcTrait for parking_lot::Mutex<T> {
    const HI_VAL: u64 = 0;
}

#[cfg(feature = "enable_spin")]
impl<T> HiarcTrait for spin::Mutex<T> {
    const HI_VAL: u64 = 0;
}

// ### ARC ###

#[derive(Debug, Default)]
pub struct HiArc<T: HiarcTrait>(Arc<T>);

impl<T: HiarcTrait> Deref for HiArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: HiarcTrait> AsRef<T> for HiArc<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: HiarcTrait> Clone for HiArc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: HiarcTrait> HiArc<T> {
    pub const HI_VAL: u64 = T::HI_VAL;

    /// Constructs a new `HiArc<T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// let five = HiArc::new(5);
    /// ```
    #[inline]
    pub fn new(data: T) -> Self {
        // Start the weak pointer count as 1 which is the weak pointer that's
        // held by all the strong pointers (kinda), see std/rc.rs for more info
        Self(Arc::new(data))
    }

    pub fn inner_arc(&self) -> &Arc<T> {
        &self.0
    }
}

// ### RC ###

#[derive(Debug, Default)]
pub struct HiRc<T: HiarcTrait>(Rc<T>);

impl<T: HiarcTrait> Deref for HiRc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: HiarcTrait> AsRef<T> for HiRc<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: HiarcTrait> Clone for HiRc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: HiarcTrait> HiRc<T> {
    pub const HI_VAL: u64 = T::HI_VAL;

    /// Constructs a new `HiRc<T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hiarc::HiRc;
    ///
    /// let five = HiRc::new(5);
    /// ```
    #[inline]
    pub fn new(data: T) -> Self {
        // Start the weak pointer count as 1 which is the weak pointer that's
        // held by all the strong pointers (kinda), see std/rc.rs for more info
        Self(Rc::new(data))
    }

    pub fn inner_rc(&self) -> &Rc<T> {
        &self.0
    }

    pub fn inner_rc_mut(&mut self) -> &mut Rc<T> {
        &mut self.0
    }
}

// ### BOX ###

#[derive(Debug, Default, Clone)]
pub struct HiBox<T: HiarcTrait>(Box<T>);

impl<T: HiarcTrait> Deref for HiBox<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: HiarcTrait> DerefMut for HiBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: HiarcTrait> AsRef<T> for HiBox<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: HiarcTrait> HiBox<T> {
    pub const HI_VAL: u64 = T::HI_VAL;

    /// Constructs a new `HiBox<T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// let five = HiBox::new(5);
    /// ```
    #[inline]
    pub fn new(data: T) -> Self {
        // Start the weak pointer count as 1 which is the weak pointer that's
        // held by all the strong pointers (kinda), see std/rc.rs for more info
        Self(Box::new(data))
    }

    pub fn inner_box(&self) -> &Box<T> {
        &self.0
    }
}

// ### Generic ###

#[derive(Debug, Default, Clone)]
pub struct Hi<T: HiarcTrait>(T);

impl<T: HiarcTrait> Hi<T> {
    pub const HI_VAL: u64 = T::HI_VAL;

    pub fn new(data: T) -> Self {
        Self(data)
    }
}

impl<T: HiarcTrait> Deref for Hi<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: HiarcTrait> DerefMut for Hi<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// this struct has nothing to do with `std::cell::RefCell`.
/// It is only useful if other Hi* components want to have a `RefCell`
/// with certain limitations. E.g. borrowing is always unsafe, so outer
/// implementations never try it or have to use `unsafe` keyword
#[derive(Debug, Default)]
pub struct HiUnsafeRefCell<T>(std::cell::RefCell<T>);

impl<T> HiUnsafeRefCell<T> {
    pub fn new(data: T) -> Self {
        Self(std::cell::RefCell::new(data))
    }

    /// # Safety
    ///
    /// Even tho this function is not unsafe in the sense of memory safety,
    /// this function is only intended to be used by macros that know what they are doing.
    /// In case you call it, you risk panics.
    pub unsafe fn borrow_mut(&self) -> std::cell::RefMut<T> {
        self.0.borrow_mut()
    }

    /// # Safety
    ///
    /// Even tho this function is not unsafe in the sense of memory safety,
    /// this function is only intended to be used by macros that know what they are doing.
    /// In case you call, it you risk panics.
    pub unsafe fn borrow(&self) -> std::cell::Ref<T> {
        self.0.borrow()
    }
}
