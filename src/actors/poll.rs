use crate::{socket::write::ZmqSocketSink, ReadHandler, SocketFd, ZmqMessage};
use actix::{
    dev::{ContextFut, ContextParts, Mailbox},
    io::WriteHandler,
    Actor, Addr, AsyncContext, StreamHandler,
};
use actix_zmq_derive::ActorContextStuff;
use std::{collections::HashMap, hash::Hash, io};

pub struct ZmqPollActorBuilder<T> {
    sockets: HashMap<T, SocketFd>,
}

impl<T> ZmqPollActorBuilder<T>
where
    T: Unpin + Hash + Eq + Copy + 'static,
{
    pub fn new() -> Self {
        let sockets = HashMap::new();
        Self { sockets }
    }

    pub fn add_socket(mut self, token: T, socket: SocketFd) -> Self {
        self.sockets.insert(token, socket);
        self
    }

    pub fn run<A>(self, actor: A) -> Addr<A>
    where
        A: Actor<Context = ZmqPollActorContext<A, T>>
            + StreamHandler<(T, ZmqMessage)>
            + ReadHandler<(T, io::Error)>
            + WriteHandler<io::Error>,
    {
        let mb = Mailbox::default();
        let parts = ContextParts::new(mb.sender_producer());

        let mut context = ZmqPollActorContext {
            parts,
            sinks: HashMap::new(),
        };

        // todo: maybe combine all read futures into SelectAll<..>
        for (token, socket) in self.sockets {
            let (stream, sink, sink_future) = socket.split();
            let stream = stream.with_token(token);

            context.sinks.insert(token, sink);
            context.spawn(sink_future);
            context.spawn(stream);
        }

        let addr = context.parts.address();
        let ctxf = ContextFut::new(context, actor, mb);

        actix::spawn(ctxf);

        addr
    }
}

#[derive(ActorContextStuff)]
pub struct ZmqPollActorContext<A: Actor<Context = Self>, T> {
    parts: ContextParts<A>,
    sinks: HashMap<T, ZmqSocketSink>,
}

// TODO:
//  - [ ] WriteHandler<(T, io::Error)>
//  - [ ] connect(endpoint)
//  - [ ] disconnect(endpoint)
//  - [ ] subscribe(topic)
//  - [ ] unsubscribe(topic)
impl<A, T> ZmqPollActorContext<A, T>
where
    A: Actor<Context = Self>,
    T: Hash + Eq + Copy,
{
    pub fn send(&mut self, token: &T, message: ZmqMessage) -> Option<ZmqMessage> {
        if let Some(sink) = self.sinks.get_mut(token) {
            sink.write(message);
            None
        } else {
            Some(message)
        }
    }
}
