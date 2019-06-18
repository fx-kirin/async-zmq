use std::sync::Arc;

use futures::{
    future::Future,
    stream::{Stream},
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
    let (_, stream) = socket.sink_stream(8192).split();

    let process = stream.fold(1, |count, _| {
            if count % 100 == 0 {
                println!("{}", count);
            }
            Ok(count + 1) as Result<_, zmq::Error>
        });

    current_thread::Runtime::new()
        .unwrap()
        .spawn(process.map(|_| ()).map_err(|_| panic!()))
        .run()
        .unwrap();
}
