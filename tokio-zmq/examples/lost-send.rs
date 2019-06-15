use std::sync::Arc;

use futures::{future::Future, stream::Stream};
use tokio::runtime::current_thread;
use tokio_zmq::{prelude::*, Dealer};

fn main() {
    env_logger::init();
    let socket = Dealer::builder(Arc::new(zmq::Context::new()))
        .bind("ipc:///tmp/lost-send")
        .build()
        .wait()
        .unwrap();
    let (sink, stream) = socket.sink_stream(8192).split();

    let receive_process = stream.forward(sink);

    current_thread::Runtime::new()
        .unwrap()
        .spawn(receive_process.map(|_| ()).map_err(|_| panic!()))
        .run()
        .unwrap();
}
