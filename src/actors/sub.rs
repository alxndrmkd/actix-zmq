use std::rc::Rc;

use actix::{
    dev::{ContextFut, ContextParts, Mailbox},
    Actor, Addr, AsyncContext,
};
use actix_zmq_derive::ActorContextStuff;

use crate::socket::{
    read::{ZmqSocketStream, ZmqStreamHandler},
    SocketFd,
};

pub trait ZmqSubActor: Actor<Context = ZmqSubActorContext<Self>> + ZmqStreamHandler {
    fn start_sub_actor(self, fd: SocketFd) -> Addr<Self> {
        let mb = Mailbox::default();
        let parts = ContextParts::new(mb.sender_producer());

        let stream = ZmqSocketStream::new(Rc::new(fd));

        let mut context = ZmqSubActorContext { parts };
        context.spawn(stream);

        let addr = context.parts.address();
        let ctxf = ContextFut::new(context, self, mb);

        actix_rt::spawn(ctxf);

        addr
    }
}

#[derive(ActorContextStuff)]
pub struct ZmqSubActorContext<A: Actor<Context = Self>> {
    parts: ContextParts<A>,
}
