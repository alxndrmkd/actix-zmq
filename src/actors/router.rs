use std::io;

use actix::{
    dev::{ContextFut, ContextParts, Mailbox},
    io::WriteHandler,
    Actor, Addr, AsyncContext,
};
use actix_zmq_derive::ActorContextStuff;

use crate::{
    message::ZmqMessage,
    socket::{read::ZmqStreamHandler, write::ZmqSocketSink, SocketFd},
};

pub trait ZmqRouterActor:
    Actor<Context = ZmqRouterActorContext<Self>> + ZmqStreamHandler + WriteHandler<io::Error>
{
    fn start_router_actor(self, fd: SocketFd) -> Addr<Self> {
        let mb = Mailbox::default();
        let parts = ContextParts::new(mb.sender_producer());

        let (stream, sink, sink_future) = fd.split();

        let mut context = ZmqRouterActorContext { parts, sink };
        context.spawn(stream);
        context.spawn(sink_future);

        let addr = context.parts.address();
        let ctxf = ContextFut::new(context, self, mb);

        actix_rt::spawn(ctxf);

        addr
    }
}

impl<A> ZmqRouterActor for A where
    A: Actor<Context = ZmqRouterActorContext<Self>> + ZmqStreamHandler + WriteHandler<io::Error>
{
}

#[derive(ActorContextStuff)]
pub struct ZmqRouterActorContext<A: Actor<Context = Self>> {
    parts: ContextParts<A>,
    sink:  ZmqSocketSink,
}

impl<A: Actor<Context = Self> + WriteHandler<io::Error>> ZmqRouterActorContext<A> {
    pub fn send(&mut self, message: ZmqMessage) {
        self.sink.write(message)
    }
}
