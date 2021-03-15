use actix::{fut::wrap_future, io::WriteHandler, Actor, ActorFutureExt, AsyncContext, Running, StreamHandler};
use actix_zmq::{ReadHandler, SocketFd, ZmqMessage, ZmqReqActor, ZmqReqActorContext, ZmqPollActorContext, ZmqPollActorBuilder};
use std::{io, io::Error, time::Duration};
use zmq::{Context as ZmqContext, REQ, ROUTER};

const CLIENT_ENDPOINT: &'static str = "tcp://0.0.0.0:50051";
const WORKER_ENDPOINT: &'static str = "tcp://0.0.0.0:50052";

fn main() -> Result<(), io::Error> {
    actix::run(async {
        let ctx = ZmqContext::new();

        let cli = SocketFd::connect(&ctx, REQ, CLIENT_ENDPOINT).expect("can't connect client socket");
        let wrk = SocketFd::connect(&ctx, REQ, WORKER_ENDPOINT).expect("can't connect client socket");

        ZmqPollActorBuilder::new()
            .add_socket(SocketId::Worker, SocketFd::bind(&ctx, ROUTER, CLIENT_ENDPOINT).expect("can't bind server socket"))
            .add_socket(SocketId::Client, SocketFd::bind(&ctx, ROUTER, WORKER_ENDPOINT).expect("can't bind server socket"))
            .run(EchoServer);

        Client.start_req_actor(cli);
        Client.start_req_actor(wrk);

        tokio::signal::ctrl_c().await.unwrap();
    })
}

/* ---------------------------------------------------------------------------------------------- */
/*                                          SERVER                                                */
/* ---------------------------------------------------------------------------------------------- */

#[derive(Debug, PartialOrd, PartialEq, Ord, Eq, Hash, Copy, Clone)]
pub enum SocketId {
    Worker,
    Client,
}

pub struct EchoServer;

impl Actor for EchoServer {
    type Context = ZmqPollActorContext<Self, SocketId>;

    fn started(&mut self, _: &mut Self::Context) {
        println!("SRV: started")
    }
}

impl WriteHandler<io::Error> for EchoServer {
    fn error(&mut self, err: Error, _: &mut Self::Context) -> Running {
        eprintln!("SRV: write error - {}", err);
        Running::Continue
    }
}

impl StreamHandler<(SocketId, ZmqMessage)> for EchoServer {
    fn handle(&mut self, item: (SocketId, ZmqMessage), ctx: &mut ZmqPollActorContext<Self, SocketId>) {
        let (socket_id, mut message) = item;

        if message.len() != 3 {
            eprintln!("unexpected message len");
            return;
        }

        let payload = message.remove(2);

        println!("SRV: recieved from {:?} - {:?}", socket_id, payload);

        let response = match String::from_utf8(payload.to_vec()) {
            Ok(v) => format!("Hello, {}!", v),
            Err(err) => format!("invalid request: {}", err),
        };

        let reply = message << response;

        if ctx.send(&socket_id, reply).is_some() {
            println!("message wasn't send")
        };
    }
}

impl ReadHandler<(SocketId, io::Error)> for EchoServer {
    fn error(&mut self, item: (SocketId, io::Error), _: &mut ZmqPollActorContext<Self, SocketId>) -> Running {
        let (socket_id, err) = item;
        eprintln!("SRV: {:?} read error - {}", socket_id, err);
        Running::Continue
    }
}

/* ---------------------------------------------------------------------------------------------- */
/*                                          CLIENT                                                */
/* ---------------------------------------------------------------------------------------------- */
struct Client;

impl Client {
    fn greet(&self, ctx: &mut ZmqReqActorContext<Self>) {
        let f = wrap_future::<_, Self>(ctx.make_request(ZmqMessage::new("ØMQ"))).map(|result, _, ctx| {
            println!("CLI: server replied - {:?}", result);
            ctx.run_later(Duration::from_secs(1), |act, ctx| act.greet(ctx));
        });

        ctx.spawn(f);
    }
}

impl Actor for Client {
    type Context = ZmqReqActorContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("client started");
        ctx.run_later(Duration::from_secs(1), |act, ctx| act.greet(ctx));
    }
}
