use std::{io, rc::Rc};

use actix::{
    dev::{ContextFut, ContextParts, Mailbox},
    io::WriteHandler,
    Actor, Addr, AsyncContext,
};
use actix_zmq_derive::ActorContextStuff;

use crate::{
    message::ZmqMessage,
    socket::{write::ZmqSocketSink, SocketFd},
};

pub trait ZmqPubActor: Actor<Context = ZmqPubActorContext<Self>> + WriteHandler<io::Error> {
    fn start_pub_actor(self, socket: SocketFd) -> Addr<Self> {
        let mb = Mailbox::default();
        let parts = ContextParts::new(mb.sender_producer());

        let (sink, sink_future) = ZmqSocketSink::new(Rc::new(socket));
        let mut context = ZmqPubActorContext { parts, sink };
        context.spawn(sink_future);

        let addr = context.parts.address();
        let ctxf = ContextFut::new(context, self, mb);

        actix_rt::spawn(ctxf);

        addr
    }
}

#[derive(ActorContextStuff)]
pub struct ZmqPubActorContext<A: Actor<Context = Self>> {
    parts: ContextParts<A>,
    sink:  ZmqSocketSink,
}

// TODO:
//  - [ ] connect(endpoint)
//  - [ ] disconnect(endpoint)
impl<A: Actor<Context = Self>> ZmqPubActorContext<A> {
    pub fn publish(&mut self, message: ZmqMessage) {
        self.sink.write(message);
    }
}
