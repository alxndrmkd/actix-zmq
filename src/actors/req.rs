use actix::{dev::ContextParts, Actor, Addr};
use actix_zmq_derive::ActorContextStuff;

use crate::{
    message::ZmqMessage,
    socket::{SocketFd, SocketRw},
};
use actix::dev::{ContextFut, Mailbox};
use std::{future::Future, io, rc::Rc};

pub trait ZmqReqActor: Actor<Context = ZmqReqActorContext<Self>> {
    fn start_req_actor(self, fd: SocketFd) -> Addr<Self> {
        let mb = Mailbox::default();
        let parts = ContextParts::new(mb.sender_producer());

        let socket = SocketRw::new(Rc::new(fd));
        let context = ZmqReqActorContext { parts, socket };

        let addr = context.parts.address();
        let ctxf = ContextFut::new(context, self, mb);

        actix_rt::spawn(ctxf);

        addr
    }
}

impl<A> ZmqReqActor for A where A: Actor<Context = ZmqReqActorContext<A>> {}

#[derive(ActorContextStuff)]
pub struct ZmqReqActorContext<A: Actor<Context = Self>> {
    parts:  ContextParts<A>,
    socket: SocketRw,
}

impl<A: Actor<Context = Self>> ZmqReqActorContext<A> {
    pub fn make_request(&mut self, request: ZmqMessage) -> impl Future<Output = io::Result<ZmqMessage>> {
        let send_request = self.socket.write(0, request);
        let read_response = self.socket.read(0);

        async {
            send_request.await?;
            read_response.await
        }
    }
}
