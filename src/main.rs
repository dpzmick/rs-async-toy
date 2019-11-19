#![feature(alloc_layout_extra)] // for some size stuff
#![feature(ptr_offset_from)]    // to do pointer offset calculations

extern crate libc;

static SIZE: usize = 2<<20;

mod pool;
mod mem;
mod util;

use crate::util::*;
use crate::mem::*;
use crate::pool::*;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};

// how does a timeout work, I want them to live in the executor and
// have timers executed intelligently
// is it possible to make futures sort of pluggable? Or, are future
// types sort of dependent on the executor used to implement them.

static VTABLE: RawWakerVTable = RawWakerVTable::new(_clone, _wake, _wake_by_ref, _drop);

unsafe fn _clone(_data: *const ()) -> RawWaker {
    RawWaker::new(std::ptr::null(), &VTABLE)
}

unsafe fn _wake(_data: *const ()) { }

unsafe fn _wake_by_ref(_data: *const ()) { }

unsafe fn _drop(_data: *const ()) { }

struct BadExecutor<'a> {
    // requires dynamic dispatch to be generic
    // can't store the tasks inline unless I really know what they all
    // are
    // FIXME not sure what to do about the output, maybe have some
    // sort of error code that brings down the entire application?
    outstanding: Vec<Pin<Box<dyn Future<Output = ()> + 'a>>>,

    // we don't do waking here, so this doesn't matter
    dummy_context: Context<'a>,
}

impl<'a> BadExecutor<'a> {
    fn new(w: &'a mut Waker) -> Self {
        Self {
            outstanding: Vec::new(),
            dummy_context: Context::from_waker(w)
        }
    }

    // in docs, the lifetime has to be 'static
    // what does it even mean for there to be a lifetime on a thing we
    // are taking by move
    fn add(&mut self, x: impl Future<Output = ()> + 'a) {
        // FIXME this pin is super sketchy and I'm not sure it is correct
        self.outstanding.push( unsafe { Pin::new_unchecked( Box::new(x) ) } )
    }

    fn run_till_complete(mut self) {
        loop {
            let next = self.outstanding.pop();
            if next.is_none() { break; }
            let mut next = next.unwrap();
            let res = next.as_mut().poll(&mut self.dummy_context); // bad error, need to pin
            if let Poll::Pending = res {
                self.outstanding.push(next);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Thing {
    a: u64,
    b: u64,
}

async fn f(pool: &Pool<'_, Thing>) {
    let mut v = Vec::new();
    for _ in 0..10 {
        let thing = pool.wait_acquire().await;
        v.push(thing);
    }
    println!("done");
}

fn main() {
    let mut map = match MappedRegion::create("test", SIZE) {
        Ok(map) => map,
        Err(errno) => {
            eprintln!("Failed to mmap huge page with {} ({})",
                      strerror(errno), errno);
            std::process::exit(-1);
        }
    };

    let mut slices = map.as_slices(&vec![
        Pool::<Thing>::required_size(10)
    ]).unwrap();

    let pool_place = slices.remove(0);
    let pool = Pool::<Thing>::from_slice(pool_place, 10); // same lifetime as mem

    let mut w = unsafe { Waker::from_raw( RawWaker::new(std::ptr::null(), &VTABLE) ) };
    let mut exec = BadExecutor::new(&mut w);
    for _ in 0..10 {
        exec.add( f(&pool) );
    }
    exec.run_till_complete();
}
