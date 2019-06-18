/*
 * This file is part of Tokio ZMQ.
 *
 * Copyright © 2018 Riley Trautman
 *
 * Tokio ZMQ is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tokio ZMQ is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tokio ZMQ.  If not, see <http://www.gnu.org/licenses/>.
 */

//! This module defines the `MultipartSinkStream` type. A wrapper around Sockets that implements
//! `futures::Sink` and `futures::Stream`.

use std::{fmt, marker::PhantomData};

use async_zmq_types::{IntoSocket, Multipart};
use futures::{task::Task, AsyncSink, Poll, Sink, Stream};

use crate::{
    async_types::{sink_type::SinkType, stream_type::StreamType},
    error::Error,
    socket::Socket,
};

/// The `MultipartSinkStream` handles sending and receiving streams of data to and from ZeroMQ
/// Sockets.
///
/// ### Example
/// ```rust
/// extern crate zmq;
/// extern crate futures;
/// extern crate tokio;
/// extern crate tokio_zmq;
///
/// use std::sync::Arc;
///
/// use futures::{Future, Sink, Stream};
/// use tokio_zmq::{prelude::*, Error, Multipart, Rep, Socket};
///
/// fn main() {
///     let context = Arc::new(zmq::Context::new());
///     let fut = Rep::builder(context)
///         .bind("tcp://*:5575")
///         .build()
///         .and_then(|rep| {
///             let sink_stream = rep.sink_stream(25);
///
///             let (sink, stream) = sink_stream.split();
///
///             stream.forward(sink)
///         });
///
///     // tokio::run(fut.map(|_| ()).map_err(|_| ()));
/// }
/// ```
pub struct MultipartSinkStream<T>
where
    T: From<Socket>,
{
    sock: Socket,
    sink: SinkType,
    stream: StreamType,
    sink_task: Option<Task>,
    stream_task: Option<Task>,
    phantom: PhantomData<T>,
}

impl<T> MultipartSinkStream<T>
where
    T: From<Socket>,
{
    pub fn new(buffer_size: usize, sock: Socket) -> Self {
        MultipartSinkStream {
            sock: sock,
            sink: SinkType::new(buffer_size),
            stream: StreamType::new(),
            sink_task: None,
            stream_task: None,
            phantom: PhantomData,
        }
    }
}

impl<T> IntoSocket<T, Socket> for MultipartSinkStream<T>
where
    T: From<Socket>,
{
    fn into_socket(self) -> T {
        T::from(self.sock)
    }
}

impl<T> Sink for MultipartSinkStream<T>
where
    T: From<Socket>,
{
    type SinkItem = Multipart;
    type SinkError = Error;

    fn start_send(
        &mut self,
        multipart: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        if self.sink_task.is_none() {
            self.sink_task = Some(futures::task::current());
        }
        self.sink
            .start_send(multipart, &self.sock, self.stream_task.as_ref())
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.sink
            .poll_complete(&self.sock, self.stream_task.as_ref())
    }
}

impl<T> Stream for MultipartSinkStream<T>
where
    T: From<Socket>,
{
    type Item = Multipart;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Multipart>, Self::Error> {
        if self.stream_task.is_none() {
            self.stream_task = Some(futures::task::current());
        }
        self.stream.poll(&self.sock, self.sink_task.as_ref())
    }
}

impl<T> fmt::Debug for MultipartSinkStream<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MultipartSinkStream")
    }
}

impl<T> fmt::Display for MultipartSinkStream<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MultipartSinkStream")
    }
}
