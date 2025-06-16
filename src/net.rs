use std::io::{Read, Write};
use std::{future::Future, os::fd::AsRawFd};
use std::net::SocketAddr;

pub struct TcpListener(std::net::TcpListener);
pub struct TcpStream(std::net::TcpStream);

struct TcpAcceptFuture<'a> {
    listener: &'a std::net::TcpListener,
    handle: Option<crate::ReactorHandle>,
}

impl Future for TcpAcceptFuture<'_> {
    type Output = std::io::Result<(TcpStream, SocketAddr)>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let listener = &self.listener;
        match self.listener.accept() {
            Ok((stream, addr)) => {
                stream.set_nonblocking(true).unwrap();
                std::task::Poll::Ready(Ok((TcpStream(stream), addr)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                self.get_mut().handle = Some(crate::register(&cx.waker(), listener.as_raw_fd(), libc::POLLIN));
                std::task::Poll::Pending
            }
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }
}

impl TcpListener {
    pub fn bind(addr: &str) -> std::io::Result<Self> {
        let listener = std::net::TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Self(listener))
    }

    pub fn accept(&self) -> impl Future<Output = std::io::Result<(TcpStream, SocketAddr)>> + '_ {
        TcpAcceptFuture { listener: &self.0, handle: None }
    }
}

struct TcpReadFuture<'a> {
    stream: &'a mut std::net::TcpStream,
    buf: &'a mut [u8],
    handle: Option<crate::ReactorHandle>,
}

impl Future for TcpReadFuture<'_> {
    type Output = std::io::Result<usize>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let stream = self.get_mut();
        match stream.stream.read(stream.buf) {
            Ok(bytes_read) => std::task::Poll::Ready(Ok(bytes_read)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                stream.handle = Some(crate::register(&cx.waker(), stream.stream.as_raw_fd(), libc::POLLIN));
                std::task::Poll::Pending
            }
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }
}

struct TcpWriteFuture<'a> {
    stream: &'a mut std::net::TcpStream,
    buf: &'a [u8],
    handle: Option<crate::ReactorHandle>,
}

impl Future for TcpWriteFuture<'_> {
    type Output = std::io::Result<usize>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let stream = self.get_mut();
        match stream.stream.write(stream.buf) {
            Ok(bytes_written) => std::task::Poll::Ready(Ok(bytes_written)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                stream.handle = Some(crate::register(&cx.waker(), stream.stream.as_raw_fd(), libc::POLLOUT));
                std::task::Poll::Pending
            }
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }
}

impl TcpStream {
    pub fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> impl Future<Output = std::io::Result<usize>> + 'a {
        TcpReadFuture { stream: &mut self.0, buf, handle: None }
    }

    pub fn write<'a>(&'a mut self, buf: &'a [u8]) -> impl Future<Output = std::io::Result<usize>> + 'a {
        TcpWriteFuture { stream: &mut self.0, buf, handle: None }
    }
}
