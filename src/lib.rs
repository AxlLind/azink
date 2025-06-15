use std::future::Future;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

unsafe fn noop_clone(_p: *const ()) -> RawWaker {
    // SAFETY: this retains all of the waker's resources, of which there are none
    RawWaker::new(ptr::null(), &NOOP_WAKER_VTABLE)
}

unsafe fn noop(_p: *const ()) {}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);
static NOOP_WAKER: Waker = unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &NOOP_WAKER_VTABLE)) };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Token(usize);

struct Task {
    token: Token,
    future: Pin<Box<dyn Future<Output = ()>>>,
    pending: bool,
}

struct Executor {
    tasks: Vec<Task>,
    token_counter: usize,
}

impl Executor {
    const fn new() -> Self {
        Executor { tasks: Vec::new(), token_counter: 0 }
    }

    fn spawn(&mut self, future: impl Future<Output = ()> + 'static) {
        let token = Token(self.token_counter);
        self.token_counter += 1;
        self.tasks.push(Task { token, future: Box::pin(future), pending: false });
    }

    fn block_on(&mut self, future: impl Future<Output = ()> + 'static) {
        self.spawn(future);
        let mut done_tasks = Vec::new();
        let mut ctx = Context::from_waker(&NOOP_WAKER);
        while !self.tasks.is_empty() {
            for task in self.tasks.iter_mut() {
                if task.pending {
                    continue;
                }
                match task.future.as_mut().poll(&mut ctx) {
                    Poll::Ready(_) => {
                        done_tasks.push(task.token);
                    }
                    Poll::Pending => {
                        task.pending = true;
                    }
                }
            }
            if !done_tasks.is_empty() {
                self.tasks.retain(|task| !done_tasks.contains(&task.token));
                done_tasks.clear();
            }
        }
    }
}

static mut EXECUTOR: Executor = Executor::new();

pub fn block_on(future: impl Future<Output = ()> + 'static) {
    unsafe { &mut *ptr::addr_of_mut!(EXECUTOR) }.block_on(future);
}

pub fn spawn(future: impl Future<Output = ()> + 'static) {
    unsafe { &mut *ptr::addr_of_mut!(EXECUTOR) }.spawn(future);
}
