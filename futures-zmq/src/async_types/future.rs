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

use std::{fmt, marker::PhantomData, mem};

use async_zmq_types::Multipart;
use futures::{Async, Future};
use log::error;

use crate::{error::Error, socket::Socket, RecvFuture, SendFuture};

pub(crate) enum SendState {
    Ready,
    Pending(Multipart),
    Running(SendFuture),
    Polling,
}

impl SendState {
    fn polling(&mut self) -> SendState {
        mem::replace(self, SendState::Polling)
    }

    fn poll_fut(&mut self, mut fut: SendFuture) -> Result<Async<()>, Error> {
        match fut.poll()? {
            Async::Ready(Some(multipart)) => {
                *self = SendState::Pending(multipart);
                Ok(Async::NotReady)
            }
            Async::Ready(None) => {
                *self = SendState::Ready;
                Ok(Async::Ready(()))
            }
            Async::NotReady => {
                *self = SendState::Running(fut);
                Ok(Async::NotReady)
            }
        }
    }

    pub(crate) fn poll_flush(&mut self, sock: &Socket) -> Result<Async<()>, Error> {
        match self.polling() {
            SendState::Ready => {
                *self = SendState::Ready;
                Ok(Async::Ready(()))
            }
            SendState::Pending(multipart) => self.poll_fut(sock.send_msg(multipart)),
            SendState::Running(fut) => self.poll_fut(fut),
            SendState::Polling => {
                error!("Called polling while polling");
                return Err(Error::Polling);
            }
        }
    }
}

pub struct MultipartRequest<T>
where
    T: From<Socket>,
{
    state: SendState,
    sock: Option<Socket>,
    phantom: PhantomData<T>,
}

impl<T> MultipartRequest<T>
where
    T: From<Socket>,
{
    pub fn new(sock: Socket, multipart: Multipart) -> Self {
        MultipartRequest {
            state: SendState::Pending(multipart),
            sock: Some(sock),
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
        let sock = self.sock.take().unwrap();

        match self.state.poll_flush(&sock)? {
            Async::Ready(_) => Ok(Async::Ready(T::from(sock))),
            Async::NotReady => {
                self.sock = Some(sock);

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
        write!(f, "SendFuture({:?})", self.sock)
    }
}

impl<T> fmt::Display for MultipartRequest<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SendFuture({:?})", self.sock)
    }
}

pub(crate) enum RecvState {
    Pending,
    Running(RecvFuture),
    Polling,
}

impl RecvState {
    fn polling(&mut self) -> RecvState {
        mem::replace(self, RecvState::Polling)
    }

    fn poll_fut(&mut self, mut fut: RecvFuture) -> Result<Async<Multipart>, Error> {
        if let ready @ Async::Ready(_) = fut.poll()? {
            *self = RecvState::Pending;
            return Ok(ready);
        }

        *self = RecvState::Running(fut);

        Ok(Async::NotReady)
    }

    pub(crate) fn poll_fetch(&mut self, sock: &Socket) -> Result<Async<Multipart>, Error> {
        match self.polling() {
            RecvState::Pending => self.poll_fut(sock.recv_msg()),
            RecvState::Running(fut) => self.poll_fut(fut),
            RecvState::Polling => {
                error!("Called polling while polling");
                return Err(Error::Polling);
            }
        }
    }
}

pub struct MultipartResponse<T>
where
    T: From<Socket>,
{
    state: RecvState,
    sock: Option<Socket>,
    phantom: PhantomData<T>,
}

impl<T> MultipartResponse<T>
where
    T: From<Socket>,
{
    pub fn new(sock: Socket) -> Self {
        MultipartResponse {
            state: RecvState::Pending,
            sock: Some(sock),
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
        let sock = self.sock.take().unwrap();

        match self.state.poll_fetch(&sock)? {
            Async::Ready(multipart) => Ok(Async::Ready((multipart, T::from(sock)))),
            Async::NotReady => {
                self.sock = Some(sock);

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
        write!(f, "RecvFuture({:?})", self.sock)
    }
}

impl<T> fmt::Display for MultipartResponse<T>
where
    T: From<Socket>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RecvFuture({:?})", self.sock)
    }
}
