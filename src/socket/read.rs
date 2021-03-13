use crate::{message::ZmqMessage, socket::SocketFd};
use actix::{Actor, ActorContext, ActorFuture, AsyncContext, Running};
use bytes::BytesMut;
use futures::Stream;
use std::{
    future::Future,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use zmq::Message;

pub trait ZmqStreamHandler
where
    Self: Actor,
    Self::Context: ActorContext,
{
    fn handle_message(&mut self, item: ZmqMessage, ctx: &mut Self::Context);
    fn handle_error(&mut self, _: io::Error, _: &mut Self::Context) -> Running {
        Running::Stop
    }
    fn started(&mut self, _: &mut Self::Context) {}
    fn finished(&mut self, ctx: &mut Self::Context) {
        ctx.stop()
    }
}

pub struct ZmqSocketRead {
    socket: Rc<SocketFd>,
    flags:  i32,
    m_buf:  Message,
    b_buf:  BytesMut,
}

impl ZmqSocketRead {
    pub fn new(socket: Rc<SocketFd>, m_buf: Message, b_buf: BytesMut, flags: i32) -> Self {
        Self {
            socket,
            flags,
            m_buf,
            b_buf,
        }
    }
}

impl Future for ZmqSocketRead {
    type Output = io::Result<ZmqMessage>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.socket.poll_read(cx, &mut this.m_buf, &mut this.b_buf, this.flags)
    }
}

impl Stream for ZmqSocketRead {
    type Item = io::Result<ZmqMessage>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        this.socket
            .poll_read(cx, &mut this.m_buf, &mut this.b_buf, this.flags)
            .map(Some)
    }
}

pub struct ZmqSocketStream(ZmqSocketRead, bool);

impl ZmqSocketStream {
    pub fn new(fd: Rc<SocketFd>) -> Self {
        let read = ZmqSocketRead::new(fd, Message::new(), BytesMut::new(), 0);
        Self(read, false)
    }
}

impl<A> ActorFuture<A> for ZmqSocketStream
where
    A: Actor + ZmqStreamHandler,
    A::Context: ActorContext + AsyncContext<A>,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, act: &mut A, ctx: &mut A::Context, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let ZmqSocketStream(read, started) = self.get_mut();

        if !*started {
            *started = true;
            <A as ZmqStreamHandler>::started(act, ctx);
        }

        match Pin::new(read).poll_next(cx) {
            Poll::Ready(Some(Ok(v))) => <A as ZmqStreamHandler>::handle_message(act, v, ctx),

            Poll::Ready(Some(Err(err))) => {
                if let Running::Stop = <A as ZmqStreamHandler>::handle_error(act, err, ctx) {
                    act.stopped(ctx);
                    return Poll::Ready(());
                }
            },

            Poll::Ready(None) => {
                <A as ZmqStreamHandler>::finished(act, ctx);
                return Poll::Ready(());
            },
            _ => {},
        };

        if !ctx.waiting() {
            cx.waker().wake_by_ref()
        }

        Poll::Pending
    }
}
