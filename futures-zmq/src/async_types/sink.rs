/*
 * This file is part of Futures ZMQ.
 *
 * Copyright © 2019 Riley Trautman
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

use std::{collections::VecDeque, fmt, marker::PhantomData};

use async_zmq_types::{IntoSocket, Multipart};
use futures::{try_ready, Async, AsyncSink, Sink};

use crate::{async_types::SendState, error::Error, socket::Socket};

pub struct MultipartSink<T>
where
    T: From<Socket>,
{
    state: SendState,
    sock: Socket,
    multiparts: VecDeque<Multipart>,
    buffer_size: usize,
    phantom: PhantomData<T>,
}

impl<T> MultipartSink<T>
where
    T: From<Socket>,
{
    pub fn new(sock: Socket, buffer_size: usize) -> Self {
        MultipartSink {
            state: SendState::Ready,
            sock,
            multiparts: VecDeque::new(),
            buffer_size,
            phantom: PhantomData,
        }
    }
}

impl<T> IntoSocket<T, Socket> for MultipartSink<T>
where
    T: From<Socket>,
{
    fn into_socket(self) -> T {
        T::from(self.sock)
    }
}

impl<T> Sink for MultipartSink<T>
where
    T: From<Socket>,
{
    type SinkItem = Multipart;
    type SinkError = Error;

    fn start_send(
        &mut self,
        multipart: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        self.poll_complete()?;

        if self.multiparts.len() >= self.buffer_size {
            return Ok(AsyncSink::NotReady(multipart));
        }

        self.multiparts.push_back(multipart);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        try_ready!(self.state.poll_flush(&self.sock));

        while let Some(multipart) = self.multiparts.pop_front() {
            self.state = SendState::Pending(multipart);
            try_ready!(self.state.poll_flush(&self.sock));
        }

        Ok(Async::Ready(()))
    }
}

impl<T> fmt::Debug for MultipartSink<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MultipartSink({:?})", self.sock)
    }
}

impl<T> fmt::Display for MultipartSink<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MultipartSink({})", self.sock)
    }
}
