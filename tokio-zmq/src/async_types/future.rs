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

//! This module contains definitions for `MultipartRequest` and `MultipartResponse`, the two types that
//! implement `futures::Future`.

use std::{fmt, marker::PhantomData};

use async_zmq_types::Multipart;
use futures::{Async, Future};

use crate::{
    async_types::{
        future_types::{request, response},
    },
    error::Error,
    socket::Socket,
};

/// The `MultipartRequest` Future handles asynchronously sending data to a socket.
///
/// ### Example
/// ```rust
/// # extern crate zmq;
/// # extern crate futures;
/// # extern crate tokio_zmq;
/// #
/// # use std::sync::Arc;
/// #
/// # use futures::Future;
/// # use tokio_zmq::{prelude::*, async_types::MultipartRequest, Error, Rep};
/// #
/// # fn main() {
/// #     get_sock();
/// # }
/// # fn get_sock() -> impl Future<Item = (), Error = Error> {
/// #     let ctx = Arc::new(zmq::Context::new());
/// #     let rep = Rep::builder(ctx)
/// #         .bind("tcp://*:5567")
/// #         .build();
/// #
/// #     rep.and_then(|rep| {
/// #       let msg = zmq::Message::from(&format!("Hey"));
/// MultipartRequest::new(rep.socket(), msg.into()).and_then(|_: Rep| {
///     // succesfull request
/// #       Ok(())
/// })
/// # })
/// # }
/// ```
pub struct MultipartRequest<T>
where
    T: From<Socket>,
{
    socks: Option<Socket>,
    multipart: Multipart,
    phantom: PhantomData<T>,
}

impl<T> MultipartRequest<T>
where
    T: From<Socket>,
{
    pub fn new(sock: Socket, multipart: Multipart) -> Self {
        MultipartRequest {
            socks: Some(sock),
            multipart: multipart,
            phantom: PhantomData,
        }
    }
}

impl<T> Future for MultipartRequest<T>
where
    T: From<Socket>,
{
    type Item = T;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        let sock = self.socks.take().ok_or(Error::Reused)?;

        match request::poll(&sock, &mut self.multipart, None)? {
            Async::Ready(()) => Ok(Async::Ready(sock.into())),
            Async::NotReady => {
                self.socks = Some(sock);

                Ok(Async::NotReady)
            }
        }
    }
}

impl<T> fmt::Debug for MultipartRequest<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SendFuture")
    }
}

impl<T> fmt::Display for MultipartRequest<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SendFuture")
    }
}

/// The `MultipartResponse` Future handles asynchronously getting data from a socket.
///
/// ### Example
/// ```rust
/// # extern crate zmq;
/// # extern crate futures;
/// # extern crate tokio_zmq;
/// #
/// # use std::sync::Arc;
/// #
/// # use futures::Future;
/// # use tokio_zmq::{prelude::*, async_types::MultipartResponse, Error, Multipart, Rep};
/// #
/// # fn main() {
/// #     get_sock();
/// # }
/// # fn get_sock() -> impl Future<Item = Multipart, Error = Error> {
/// #     let ctx = Arc::new(zmq::Context::new());
/// #     let rep = Rep::builder(ctx)
/// #         .bind("tcp://*:5567")
/// #         .build();
/// #     rep.and_then(|rep| {
/// MultipartResponse::new(rep.socket()).and_then(|(multipart, _): (_, Rep)| {
///     // handle multipart response
///     # Ok(multipart)
/// })
/// # })
/// # }
/// ```
pub struct MultipartResponse<T>
where
    T: From<Socket>,
{
    socks: Option<Socket>,
    multipart: Multipart,
    phantom: PhantomData<T>,
}

impl<T> MultipartResponse<T>
where
    T: From<Socket>,
{
    pub fn new(sock: Socket) -> Self {
        MultipartResponse {
            socks: Some(sock),
            multipart: Multipart::new(),
            phantom: PhantomData,
        }
    }
}

impl<T> Future for MultipartResponse<T>
where
    T: From<Socket>,
{
    type Item = (Multipart, T);
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        let sock = self.socks.take().ok_or(Error::Reused)?;

        match response::poll(&sock, &mut self.multipart, None)? {
            Async::Ready(multipart) => Ok(Async::Ready((
                multipart,
                sock.into(),
            ))),
            Async::NotReady => {
                self.socks = Some(sock);

                Ok(Async::NotReady)
            }
        }
    }
}

impl<T> fmt::Debug for MultipartResponse<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RecvFuture")
    }
}

impl<T> fmt::Display for MultipartResponse<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RecvFuture")
    }
}
