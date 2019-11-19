use std::alloc::Layout;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

// FIXME audit for correctness, very suspicious
// FIXME use a better method for dealing with concurrency

pub struct PoolSlot<'a, T> {
    // inline for fast access
    used: &'a AtomicBool,
    val:  &'a mut T,
}

impl<'a, T> Deref for PoolSlot<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target { &*self.val }
}

impl<'a, T> DerefMut for PoolSlot<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut *self.val }
}

impl<'a, T> Drop for PoolSlot<'a, T> {
    fn drop(&mut self) {
        self.used.store(false, Ordering::Release);
    }
}

// ????? what are the borrowing rules for this thing
pub struct PoolFuture<'a, T> {
    pool: Pool<'a, T>, // owned pool, see below for why
}

impl<'a, T: Copy> Future for PoolFuture<'a, T> {
    type Output = PoolSlot<'a, T>;

    fn poll(self: Pin<&mut Self>, _ctx: &mut Context) -> Poll<Self::Output> {
        // make another attempt on the pool, if success, return the slot
        // if not, this future will try again later, initially busy wait
        let slot = self.pool.acquire();
        match slot {
            Some(slot) => Poll::Ready(slot),
            None       => Poll::Pending,
        }

        // need some sort of 'waker' that will do the execution of
        // this future
    }
}

// The pool doesn't own any of the data, it just manages it
// The lifetime 'a represents the lifetime of the data segment
pub struct Pool<'a, T> {
    n_elem:    usize,           // client side, maybe should also put this in shared memory?
    used_list: *mut AtomicBool, // array in shared memory (array of bools has same layout as array of AtomicBool)
    objects:   *mut T,          // array in shared memory

    // FIXME could both members be slices of references?
    // wouldn't need the phantom then either

    // the pool can't live longer than the memory that holds the
    // actual elements
    data_seg_lt: PhantomData<&'a T>,
}

// requiring copy because copy types can be duplicated with just a
// memcpy. essentially this says that there's no pointers in them.
impl<'a, T: 'a + Copy> Pool<'a, T> {
    // return Layout, byte_offset from start for objects
    fn layout(size: usize) -> (Layout, usize) {
        // requiring nightly to get this handy feature
        // FIXME(nightly)
        let used_list = std::alloc::Layout::array::<AtomicBool>(size).unwrap();
        let objs  = std::alloc::Layout::array::<T>(size).unwrap();

        return used_list.extend(objs).unwrap();
    }

    pub fn required_size(size: usize) -> usize { Self::layout(size).0.size() }

    pub fn required_align() -> usize { Self::layout(1 /* doesn't matter here */).0.align() }

    /// pointer must have at least Self::required_size::<T>() space.
    /// unsafe, the pool must not live longer than the memory the pool
    /// is associated with
    pub fn from_slice(data: &'a mut [u8], n_elements: usize) -> Self {
        // FIXME check slice size
        // FIXME check slice alignment
        // FIXME is a slice the right way to do this (thinking no, but the lifetime tracking is useful)
        unsafe {
            let ptr = data.as_mut_ptr();
            let (_, off) = Self::layout(n_elements);

            Self {
                n_elem:      n_elements,
                used_list:   ptr as *mut AtomicBool,
                objects:     ptr.add(off) as *mut T,
                data_seg_lt: PhantomData
            }
        }
    }

    // FIXME add reset method the resets the pool

    // lifetime should be the same as self
    // walk the free list looking for something we can use
    // FIXME mutablility is questionable, the _pool_ isn't actually
    // modified in any way, just the memory segment. made this
    // non-mutable so I can run more than a single future at once
    pub fn acquire(&self) -> Option<PoolSlot<'a, T>> {
        for i in 0..self.n_elem {
            if unsafe { (*self.used_list.add(i)).compare_and_swap(false, true, Ordering::AcqRel) } == false {
                // we got one
                return Some(PoolSlot {
                    val:      unsafe { &mut *self.objects.add(i) },
                    used:     unsafe { &*self.used_list.add(i) },
                });
            }
        }

        // FIXME should default initialize the thing

        return None;
    }

    pub fn wait_acquire(&self) -> PoolFuture<'a, T> {
        // we make a new pool here, owned by the future
        // this allows the future to live for longer that the the pool
        // that created it. This seems funky, and I'm curious how this
        // is done in other places. In this case, it sort of makes
        // sense since the lifetime of the generated objects is
        // related to the memory region the pool manages, not the
        // lifetime of the pool.
        PoolFuture {
            pool: Pool {
                n_elem:      self.n_elem,     // copy
                used_list:   self.used_list,  // copy of pointer
                objects:     self.objects,    // copy of pointer
                data_seg_lt: PhantomData,
            }
        }
    }
}

// FIXME how to enforce the each poolslot object comes from the pool
// associated with this memory region.
// currently this is unsafe, could cause memory corruption if we
// crossed pools.
