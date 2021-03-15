use actix::{fut::wrap_future, io::WriteHandler, Actor, ActorFutureExt, AsyncContext, Running, StreamHandler};
use actix_zmq::{
    ReadHandler, SocketFd, ZmqAsyncActor, ZmqAsyncActorContext, ZmqMessage, ZmqReqActor, ZmqReqActorContext,
};
use std::{io, io::Error, time::Duration};
use zmq::{Context as ZmqContext, REQ, ROUTER};

const ENDPOINT: &'static str = "tcp://0.0.0.0:50051";

fn main() -> Result<(), io::Error> {
    actix::run(async {
        let ctx = ZmqContext::new();

        let srv = SocketFd::bind(&ctx, ROUTER, ENDPOINT).expect("can't bind server socket");
        let cli = SocketFd::connect(&ctx, REQ, ENDPOINT).expect("can't connect client socket");

        EchoServer.start_async_actor(srv);
        Client.start_req_actor(cli);

        tokio::signal::ctrl_c().await.unwrap();
    })
}

/* ---------------------------------------------------------------------------------------------- */
/*                                          SERVER                                                */
/* ---------------------------------------------------------------------------------------------- */

pub struct EchoServer;

impl Actor for EchoServer {
    type Context = ZmqAsyncActorContext<Self>;

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

impl StreamHandler<ZmqMessage> for EchoServer {
    fn handle(&mut self, mut message: ZmqMessage, ctx: &mut ZmqAsyncActorContext<Self>) {
        if message.len() != 3 {
            eprintln!("unexpected message len");
            return;
        }

        let payload = message.remove(2);

        println!("SRV: recieved from client - {:?}", payload);

        let response = match String::from_utf8(payload.to_vec()) {
            Ok(v) => format!("Hello, {}!", v),
            Err(err) => format!("invalid request: {}", err),
        };

        let reply = message << response;

        ctx.send(reply);
    }
}

impl ReadHandler<io::Error> for EchoServer {
    fn error(&mut self, err: Error, _: &mut ZmqAsyncActorContext<Self>) -> Running {
        eprintln!("SRV: read error - {}", err);
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
