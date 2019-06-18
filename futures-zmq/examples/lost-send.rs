use std::sync::Arc;

use failure::Error;
use futures::{
    future::Future,
    stream::{repeat, Stream},
};
use tokio::runtime::current_thread;
use futures_zmq::{prelude::*, Dealer};

fn main() {
    std::env::set_var("RUST_LOG", "futures_zmq=info");
    env_logger::init();

    let socket = Dealer::builder(Arc::new(zmq::Context::new()))
        .connect("ipc:///tmp/lost-send")
        .build()
        .wait()
        .unwrap();
    let (sink, _) = socket.sink_stream(8192).split();

    let process = repeat(0)
        .map(|_| zmq::Message::from("hi").into())
        .forward(sink);

    current_thread::Runtime::new()
        .unwrap()
        .spawn(process.map(|_| ()).map_err(|_: Error| panic!()))
        .run()
        .unwrap();
}
