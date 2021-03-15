use std::io;

use actix::{
    dev::{ContextFut, ContextParts, Mailbox},
    io::WriteHandler,
    Actor, Addr, AsyncContext, StreamHandler,
};
use actix_zmq_derive::ActorContextStuff;

use crate::{
    message::ZmqMessage,
    socket::{read::ReadHandler, write::ZmqSocketSink, SocketFd},
};

pub trait ZmqAsyncActor:
    Actor<Context = ZmqAsyncActorContext<Self>>
    + StreamHandler<ZmqMessage>
    + ReadHandler<io::Error>
    + WriteHandler<io::Error>
{
    fn start_async_actor(self, fd: SocketFd) -> Addr<Self> {
        let mb = Mailbox::default();
        let parts = ContextParts::new(mb.sender_producer());

        let (stream, sink, sink_future) = fd.split();

        let mut context = ZmqAsyncActorContext { parts, sink };
        context.spawn(stream);
        context.spawn(sink_future);

        let addr = context.parts.address();
        let ctxf = ContextFut::new(context, self, mb);

        actix_rt::spawn(ctxf);

        addr
    }
}

impl<A> ZmqAsyncActor for A where
    A: Actor<Context = ZmqAsyncActorContext<Self>>
        + StreamHandler<ZmqMessage>
        + ReadHandler<io::Error>
        + WriteHandler<io::Error>
{
}

#[derive(ActorContextStuff)]
pub struct ZmqAsyncActorContext<A: Actor<Context = Self>> {
    parts: ContextParts<A>,
    sink:  ZmqSocketSink,
}

// TODO:
//  - [ ] connect(endpoint)
//  - [ ] disconnect(endpoint)
impl<A: Actor<Context = Self> + WriteHandler<io::Error>> ZmqAsyncActorContext<A> {
    pub fn send(&mut self, message: ZmqMessage) {
        self.sink.write(message)
    }
}
