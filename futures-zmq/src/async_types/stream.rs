/*
 * This file is part of Futures ZMQ.
 *
 * Copyright © 2018 Riley Trautman
 *
 * Futures ZMQ is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Futures ZMQ is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Futures ZMQ.  If not, see <http://www.gnu.org/licenses/>.
 */

use std::{fmt, marker::PhantomData};

use async_zmq_types::{IntoSocket, Multipart};
use futures::{try_ready, Async, Stream};

use crate::{async_types::RecvState, error::Error, socket::Socket};

pub struct MultipartStream<T>
where
    T: From<Socket>,
{
    state: RecvState,
    sock: Socket,
    phantom: PhantomData<T>,
}

impl<T> MultipartStream<T>
where
    T: From<Socket>,
{
    pub fn new(sock: Socket) -> Self {
        MultipartStream {
            state: RecvState::Pending,
            sock,
            phantom: PhantomData,
        }
    }
}

impl<T> IntoSocket<T, Socket> for MultipartStream<T>
where
    T: From<Socket>,
{
    fn into_socket(self) -> T {
        T::from(self.sock)
    }
}

impl<T> Stream for MultipartStream<T>
where
    T: From<Socket>,
{
    type Item = Multipart;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        let mpart = try_ready!(self.state.poll_fetch(&self.sock));

        Ok(Async::Ready(Some(mpart)))
    }
}

impl<T> fmt::Debug for MultipartStream<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MultipartStream({:?})", self.sock)
    }
}

impl<T> fmt::Display for MultipartStream<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MultipartStream({})", self.sock)
    }
}
