use std::sync::Arc;

use futures::{
    future::Future,
    stream::{iter_ok, Stream},
};
use tokio::runtime::current_thread;
use futures_zmq::{prelude::*, Dealer};

fn main() {
    env_logger::init();
    let socket = Dealer::builder(Arc::new(zmq::Context::new()))
        .connect("ipc:///tmp/lost-send")
        .build()
        .wait()
        .unwrap();
    let (sink, stream) = socket.sink_stream(8192).split();

    let send_process = iter_ok(0..5000)
        .map(|_| zmq::Message::from("hi").into())
        .forward(sink)
        .and_then(|_| {
            stream.chunks(100).take(50).fold(1, |count, _| {
                println!("{}", count * 100);
                Ok(count + 1) as Result<_, zmq::Error>
            })
        });

    current_thread::Runtime::new()
        .unwrap()
        .spawn(send_process.map(|_| ()).map_err(|_| panic!()))
        .run()
        .unwrap();
}
