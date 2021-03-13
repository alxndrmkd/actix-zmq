use crate::{message::ZmqMessage, socket::SocketFd};
use actix::{io::WriteHandler, Actor, ActorFuture, Running};
use futures::{Future, Sink};
use std::{
    cell::RefCell,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

pub struct ZmqSocketWrite {
    socket:  Rc<SocketFd>,
    flags:   i32,
    message: ZmqMessage,
}

impl ZmqSocketWrite {
    pub fn new(socket: Rc<SocketFd>, flags: i32, message: ZmqMessage) -> Self {
        Self { socket, flags, message }
    }
}

impl Future for ZmqSocketWrite {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.socket.poll_write(cx, &mut this.message, this.flags)
    }
}

impl Sink<ZmqMessage> for ZmqSocketWrite {
    type Error = io::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Future::poll(self, cx)
    }

    fn start_send(self: Pin<&mut Self>, item: ZmqMessage) -> Result<(), Self::Error> {
        let this = self.get_mut();
        this.message = item;

        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Future::poll(self, cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Future::poll(self, cx)
    }
}

struct SinkInner {
    write:    ZmqSocketWrite,
    waker:    Option<Waker>,
    stopping: bool,
    buf:      Vec<ZmqMessage>,
}

pub struct ZmqSocketSink {
    inner: Rc<RefCell<SinkInner>>,
}

impl ZmqSocketSink {
    pub fn new(fd: Rc<SocketFd>) -> (Self, ZmqSocketSinkFuture) {
        let write = ZmqSocketWrite::new(fd, 0, ZmqMessage::default());
        let waker = None;
        let stopping = false;
        let buf = Vec::new();

        let inner = Rc::new(RefCell::new(SinkInner {
            write,
            waker,
            stopping,
            buf,
        }));

        let future = ZmqSocketSinkFuture { inner: inner.clone() };

        let sink = Self { inner };

        (sink, future)
    }

    pub fn write(&self, message: ZmqMessage) {
        self.inner.borrow_mut().buf.push(message);
    }
}

pub struct ZmqSocketSinkFuture {
    inner: Rc<RefCell<SinkInner>>,
}

impl<A: Actor + WriteHandler<io::Error>> ActorFuture<A> for ZmqSocketSinkFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, act: &mut A, ctx: &mut A::Context, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let inner = &mut this.inner.borrow_mut();

        if let Poll::Ready(Ok(())) = Pin::new(&mut inner.write).poll_ready(cx) {
            if let Some(next) = inner.buf.pop() {
                let _ = Pin::new(&mut inner.write).start_send(next);
            }
        }

        if inner.stopping {
            match Pin::new(&mut inner.write).poll_close(cx) {
                Poll::Ready(Err(err)) => {
                    if let Running::Stop = act.error(err, ctx) {
                        act.finished(ctx);
                        return Poll::Ready(());
                    }
                },

                Poll::Ready(Ok(())) => {
                    if inner.buf.is_empty() {
                        act.finished(ctx);
                        return Poll::Ready(());
                    }
                },

                _ => {},
            }
        } else if let Poll::Ready(Err(err)) = Pin::new(&mut inner.write).poll_flush(cx) {
            if let Running::Stop = act.error(err, ctx) {
                act.stopped(ctx);
                return Poll::Ready(());
            }
        }

        inner.waker.replace(cx.waker().clone());

        Poll::Pending
    }
}
