use std::rc::Rc;

use actix::{
    dev::{ContextFut, ContextParts, Mailbox},
    Actor, Addr, AsyncContext, StreamHandler,
};
use actix_zmq_derive::ActorContextStuff;

use crate::{
    socket::{
        read::{ReadHandler, ZmqSocketStream},
        SocketFd,
    },
    ZmqMessage,
};
use std::io;

pub trait ZmqSubActor:
    Actor<Context = ZmqSubActorContext<Self>> + StreamHandler<ZmqMessage> + ReadHandler<io::Error>
{
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

// TODO:
//  - [ ] connect(endpoint)
//  - [ ] disconnect(endpoint)
//  - [ ] subscribe(topic)
//  - [ ] unsubscribe(topic)

#[derive(ActorContextStuff)]
pub struct ZmqSubActorContext<A: Actor<Context = Self>> {
    parts: ContextParts<A>,
}
