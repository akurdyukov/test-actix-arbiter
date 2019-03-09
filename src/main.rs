
#[macro_use]
extern crate actix;
extern crate futures;
extern crate tokio;
#[macro_use]
extern crate log;
extern crate simple_logger;
extern crate futures_locks;
extern crate rand;

use std::{thread, time};
use std::collections::HashMap;
use futures::{Future, future};
use actix::prelude::*;
use actix::msgs::StartActor;
use futures_locks::RwLock;
use rand::prelude::*;

const CHILD_COUNT: u16 = 10;

#[derive(Debug)]
struct RoutedMessage {
    receiver: u16,
}

impl RoutedMessage {
    fn new(receiver: u16) -> Self {
        RoutedMessage {
            receiver
        }
    }
}

impl Message for RoutedMessage {
    type Result = ();
}

// This actor takes 5 sec to start
struct SlowStartingActor {
    id: u16,
}

impl SlowStartingActor {
    fn new(id: u16) -> Self {
        SlowStartingActor {
            id
        }
    }
}

impl Actor for SlowStartingActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        debug!("Starting slow loading actor {}", self.id);
        let lag = time::Duration::from_millis(1000);
        thread::sleep(lag);
        debug!("Slow loading actor {} started", self.id);
    }
}

impl Handler<RoutedMessage> for SlowStartingActor {
    type Result = ();

    fn handle(&mut self, msg: RoutedMessage, _: &mut Self::Context) -> Self::Result {
        debug!("Slow starting got routed message: {:?}", &msg);
    }
}

struct SupervisorActor {
    addr: RwLock<HashMap<u16, Addr<SlowStartingActor>>>,
}

impl SupervisorActor {
    fn new() -> Self {
        SupervisorActor {
            addr: RwLock::new(HashMap::new()),
        }
    }
}

impl Actor for SupervisorActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Starting supervisor");
        let arbiter = Arbiter::builder()
            .name("separate")
            .stop_system_on_panic(true)
            .build();

        let local_arbiter = arbiter.clone();
        let f = self.addr.write().and_then(move |mut guard| {
            debug!("Write lock aquired");

            let starters = {
                let mut fu = vec![];

                for i in 0..CHILD_COUNT {
                    let index = i;
                    let start_msg = StartActor::new(move |_ctx| {
                        SlowStartingActor::new(index)
                    });

                    fu.push(local_arbiter.send(start_msg)
                            .map(move |addr| (addr, index))
                            .map_err(|err| error!("Error creating child: {:?}", err)))
                }
                fu
            };

            future::join_all(starters)
                .map(move |addrs| {
                    for (addr, index) in addrs {
                        guard.insert(index, addr);
                    }
                    debug!("Supervisor got child's addresses");
                })
        }).into_actor(self);

        ctx.spawn(f);
    }
}

impl Handler<RoutedMessage> for SupervisorActor {
    type Result = ();

    fn handle(&mut self, msg: RoutedMessage, ctx: &mut Self::Context) -> Self::Result {
        debug!("Supervisor got message");
        let f = self.addr.read().map(|guard| {
                        debug!("Supervisor read lock aquired");
                        let addr = guard.get(&msg.receiver).clone();
                        match addr {
                            Some(a) => {
                                let r = msg.receiver;
                                a.do_send(msg);
                                debug!("Message sent to child {}", r);
                            },
                            None => panic!("No child {} found", msg.receiver),
                        }
                    })
                    .into_actor(self);
        ctx.spawn(f);
    }
}

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    System::run(|| {
        let actor = SupervisorActor::new().start();

        let mut rng = thread_rng();
        let n: u16 = rng.gen_range(0, CHILD_COUNT);

        tokio::spawn(
            actor.send(RoutedMessage::new(n))
                .map(|_| {
                    debug!("Routed message sent to supervisor");
                })
                .map_err(|_| ()),
        );
    });
}
