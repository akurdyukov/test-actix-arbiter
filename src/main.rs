
#[macro_use]
extern crate actix;
extern crate futures;
extern crate tokio;
#[macro_use]
extern crate log;
extern crate simple_logger;

use std::{thread, time};
use futures::Future;
use actix::prelude::*;

struct RoutedMessage;

impl RoutedMessage {
    fn new() -> Self {
        RoutedMessage {}
    }
}

impl Message for RoutedMessage {
    type Result = ();
}

// This actor takes 5 sec to start
struct SlowStartingActor {
}

impl SlowStartingActor {
    fn new() -> Self {
        SlowStartingActor {}
    }
}

impl Actor for SlowStartingActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        debug!("Starting slow loading actor");
        let lag = time::Duration::from_millis(1000);
        thread::sleep(lag);
        debug!("Slow loading actor started");
    }
}

impl Handler<RoutedMessage> for SlowStartingActor {
    type Result = ();

    fn handle(&mut self, msg: RoutedMessage, _: &mut Self::Context) -> Self::Result {
        debug!("Slow starting got routed message");
    }
}

struct SupervisorActor {
    addr: Option<Addr<SlowStartingActor>>,
}

impl SupervisorActor {
    fn new() -> Self {
        SupervisorActor {
            addr: None,
        }
    }
}

impl Actor for SupervisorActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        debug!("Starting arbiter");
        self.addr = Some(SlowStartingActor::new().start());
    }
}

impl Handler<RoutedMessage> for SupervisorActor {
    type Result = ();

    fn handle(&mut self, msg: RoutedMessage, _: &mut Self::Context) -> Self::Result {
        debug!("Supervisor got message");
        let addr = self.addr.clone().unwrap();
        addr.do_send(msg);
    }
}

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    System::run(|| {
        let actor = SupervisorActor::new().start();

        tokio::spawn(
            actor.send(RoutedMessage::new())
                .map(|_| {
                    debug!("Finish!");
                    System::current().stop();
                })
                .map_err(|_| ()),
        );
    });
}
