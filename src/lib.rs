use std::future::Future;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub mod net;

unsafe fn noop_clone(_p: *const ()) -> RawWaker {
    // SAFETY: this retains all of the waker's resources, of which there are none
    RawWaker::new(ptr::null(), &NOOP_WAKER_VTABLE)
}

unsafe fn noop(_p: *const ()) {}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Token(usize);

struct Task {
    token: Token,
    future: Pin<Box<dyn Future<Output = ()>>>,
    pending: bool,
}

struct Reactor {
    tokens: Vec<Token>,
    poll_fds: Vec<libc::pollfd>,
}

impl Reactor {
    const fn new() -> Self {
        Self { tokens: Vec::new(), poll_fds: Vec::new() }
    }

    fn register(&mut self, token: Token, fd: RawFd, events: i16) {
        self.tokens.push(token);
        self.poll_fds.push(libc::pollfd {
            fd,
            events,
            revents: 0,
        });
    }

    fn unregister(&mut self, token: Token) {
        if let Some(pos) = self.tokens.iter().position(|&t| t == token) {
            self.tokens.remove(pos);
            self.poll_fds.remove(pos);
        }
    }

    fn poll(&mut self) -> std::io::Result<Vec<Token>> {
        let res = unsafe {
            libc::poll(self.poll_fds.as_mut_ptr(), self.poll_fds.len() as _, 100)
        };
        if res < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if res == 0 {
            return Ok(Vec::new()); // No events ready
        }
        let ready_tokens = self.poll_fds.iter().enumerate()
            .filter(|(_, poll_fd)| poll_fd.revents & poll_fd.events != 0)
            .map(|(i, _)| self.tokens[i])
            .collect();
        Ok(ready_tokens)
    }
}

struct Executor {
    tasks: Vec<Task>,
    reactor: Reactor,
    token_counter: usize,
}

impl Executor {
    const fn new() -> Self {
        Executor { tasks: Vec::new(), reactor: Reactor::new(), token_counter: 0 }
    }

    unsafe fn get() -> &'static mut Self {
        static mut EXECUTOR: Executor = Executor::new();
        unsafe { &mut *ptr::addr_of_mut!(EXECUTOR) }
    }

    fn spawn(&mut self, future: impl Future<Output = ()> + 'static) {
        let token = Token(self.token_counter);
        self.token_counter += 1;
        self.tasks.push(Task { token, future: Box::pin(future), pending: false });
    }

    fn block_on(&mut self, future: impl Future<Output = ()> + 'static) {
        self.spawn(future);
        let mut done_tasks = Vec::new();
        while !self.tasks.is_empty() {
            for task in self.tasks.iter_mut() {
                if task.pending {
                    continue;
                }
                let waker = unsafe { Waker::from_raw(RawWaker::new(task.token.0 as _, &NOOP_WAKER_VTABLE)) };
                match task.future.as_mut().poll(&mut Context::from_waker(&waker)) {
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
            let ready_tokens = self.reactor.poll().unwrap();
            for t in ready_tokens {
                if let Some(task) = self.tasks.iter_mut().find(|task| task.token == t) {
                    task.pending = false;
                }
            }
        }
    }
}

pub fn block_on(future: impl Future<Output = ()> + 'static) {
    unsafe { Executor::get() }.block_on(future);
}

pub fn spawn(future: impl Future<Output = ()> + 'static) {
    unsafe { Executor::get() }.spawn(future);
}

pub struct ReactorHandle(Token);

impl Drop for ReactorHandle {
    fn drop(&mut self) {
        unsafe { Executor::get() }.reactor.unregister(self.0);
    }
}

pub fn register(waker: &Waker, fd: RawFd, events: i16) -> ReactorHandle {
    let token = Token(waker.data() as usize);
    unsafe { Executor::get() }.reactor.register(token, fd, events);
    ReactorHandle(token)
}

pub fn unregister(waker: &Waker) {
    let token = Token(waker.data() as usize);
    unsafe { Executor::get() }.reactor.unregister(token);
}
