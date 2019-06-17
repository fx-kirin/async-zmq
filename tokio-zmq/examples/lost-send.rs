use std::sync::Arc;

use failure::{Error, Fail};
use futures::{future::Future, stream::Stream, sync::mpsc::channel};
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

    let (tx, rx) = channel(8192);

    let receive_process = stream.from_err::<Error>().forward(tx);
    let send_process = rx.map_err(|_| RecvError).from_err::<Error>().forward(sink);

    current_thread::Runtime::new()
        .unwrap()
        .spawn(send_process.map(|_| ()).map_err(|_| panic!()))
        .spawn(receive_process.map(|_| ()).map_err(|_| panic!()))
        .run()
        .unwrap();
}

#[derive(Clone, Debug, Fail)]
#[fail(display = "Failed to receive data from mpsc")]
struct RecvError;
