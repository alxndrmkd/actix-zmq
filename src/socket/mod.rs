pub mod read;
pub mod write;

use bytes::BytesMut;
use std::{
    io,
    os::unix::io::RawFd,
    rc::Rc,
    task::{Context, Poll},
};
use tokio::io::unix::AsyncFd;
use zmq::{Context as ZmqContext, Message, PollEvents, Socket, SocketType, DONTWAIT, POLLIN, POLLOUT, SNDMORE};

use crate::{
    message::ZmqMessage,
    socket::{
        read::{ZmqSocketRead, ZmqSocketStream},
        write::{ZmqSocketSink, ZmqSocketSinkFuture, ZmqSocketWrite},
    },
};

pub struct SocketFd(Socket, AsyncFd<RawFd>);

impl SocketFd {
    pub fn connect(ctx: &ZmqContext, typ: SocketType, ep: &str) -> io::Result<Self> {
        let sock = ctx.socket(typ)?;
        sock.connect(ep)?;

        let fd = sock.get_fd()?;
        let fd = AsyncFd::new(fd)?;

        Ok(SocketFd(sock, fd))
    }

    pub fn bind(ctx: &ZmqContext, typ: SocketType, ep: &str) -> io::Result<Self> {
        let sock = ctx.socket(typ)?;
        sock.bind(ep)?;

        let fd = sock.get_fd()?;
        let fd = AsyncFd::new(fd)?;

        Ok(SocketFd(sock, fd))
    }

    pub fn split(self) -> (ZmqSocketStream, ZmqSocketSink, ZmqSocketSinkFuture) {
        let fd = Rc::new(self);
        let stream = ZmqSocketStream::new(fd.clone());
        let (sink, sink_future) = ZmqSocketSink::new(fd);

        (stream, sink, sink_future)
    }

    pub fn poll_read(
        &self,
        cx: &mut Context<'_>,
        m_buf: &mut Message,
        b_buf: &mut BytesMut,
        flags: i32,
    ) -> Poll<io::Result<ZmqMessage>> {
        let SocketFd(sock, _) = self;

        match self.poll(POLLIN, cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(v) => v?,
        };

        let mut buf = ZmqMessage::default();

        loop {
            let has_more;

            match sock.recv(m_buf, flags | DONTWAIT) {
                Ok(_) => {
                    has_more = m_buf.get_more();
                    b_buf.extend(m_buf.as_ref());

                    let part = b_buf.split().freeze();

                    buf <<= part;
                },

                Err(err) => return Poll::Ready(Err(err.into())),
            }

            if !has_more {
                break;
            }
        }

        Poll::Ready(Ok(buf))
    }

    pub fn poll_write(&self, cx: &mut Context<'_>, message: &mut ZmqMessage, flags: i32) -> Poll<io::Result<()>> {
        let SocketFd(sock, _) = self;

        match self.poll(POLLOUT, cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(v) => v?,
        };

        let parts_count = message.len();

        for ix in 0..parts_count {
            let send_result = if ix == parts_count - 1 {
                sock.send(message[0].as_ref(), flags | DONTWAIT)
            } else {
                sock.send(message[0].as_ref(), flags | DONTWAIT | SNDMORE)
            };

            match send_result {
                Err(zmq::Error::EAGAIN) if message.len() == parts_count => {
                    return Poll::Pending;
                },

                Err(err) => return Poll::Ready(Err(io::Error::from(err))),

                _ => {
                    message.remove(0);
                },
            }
        }

        Poll::Ready(Ok(()))
    }

    fn poll(&self, events: PollEvents, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let SocketFd(sock, fd) = self;

        if (sock.get_events()? & events) == events {
            Poll::Ready(Ok(()))
        } else {
            if let Poll::Ready(mut guard) = fd.poll_read_ready(cx)? {
                guard.clear_ready();
                cx.waker().wake_by_ref();
            }

            Poll::Pending
        }
    }
}

pub struct SocketRw {
    socket: Rc<SocketFd>,
    b_buf:  BytesMut,
}

impl SocketRw {
    pub fn new(socket: Rc<SocketFd>) -> Self {
        let b_buf = BytesMut::new();
        SocketRw { socket, b_buf }
    }

    pub fn read(&mut self, flags: i32) -> ZmqSocketRead {
        ZmqSocketRead::new(self.socket.clone(), Message::new(), self.b_buf.split(), flags)
    }

    pub fn write(&self, flags: i32, message: ZmqMessage) -> ZmqSocketWrite {
        ZmqSocketWrite::new(self.socket.clone(), flags, message)
    }
}
