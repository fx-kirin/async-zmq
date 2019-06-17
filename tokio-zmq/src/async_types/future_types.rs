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

//! This module contains definitions for `RequestFuture` and `ResponseFuture`, the two types that
//! implement `futures::Future`.

/*-------------------------------RequestFuture--------------------------------*/

pub(crate) mod request {
    use async_zmq_types::Multipart;
    use futures::{try_ready, Async, Poll};
    use log::{debug, error};
    use mio::Ready;
    use zmq::{self, Message, DONTWAIT, SNDMORE};

    use crate::{error::Error, Socket};

    fn send(sock: &Socket, multipart: &mut Multipart) -> Poll<(), Error> {
        while let Some(msg) = multipart.pop_front() {
            match send_msg(sock, msg, multipart.is_empty())? {
                Some(msg) => {
                    multipart.push_front(msg);
                    return Ok(Async::NotReady);
                }
                None => continue,
            }
        }

        Ok(Async::Ready(()))
    }

    fn send_msg(sock: &Socket, msg: Message, last: bool) -> Result<Option<Message>, Error> {
        let flags = DONTWAIT | if last { 0 } else { SNDMORE };

        let msg_clone = Message::from(&*msg);

        match sock.send_msg(msg, flags) {
            Ok(_) => Ok(None),
            Err(zmq::Error::EAGAIN) => {
                // return message in future
                debug!("RequestFuture: EAGAIN");
                Ok(Some(msg_clone))
            }
            Err(e) => {
                error!("Send error: {}", e);
                Err(e.into())
            }
        }
    }

    pub(crate) fn poll(
        sock: &Socket,
        multipart: &mut Multipart,
    ) -> Poll<(), Error> {
        let ready = Ready::readable();
        try_ready!(sock.poll_write_ready());

        match send(sock, multipart)? {
            Async::Ready(()) => {
                Ok(Async::Ready(()))
            }
            Async::NotReady => {
                sock.clear_ready(ready)?;
                Ok(Async::NotReady)
            }
        }
    }
}

/*-------------------------------ResponseFuture-------------------------------*/

pub(crate) mod response {
    use std::mem;

    use async_zmq_types::Multipart;
    use futures::{try_ready, Async, Poll};
    use log::{debug, error};
    use mio::Ready;
    use zmq::{self, Message};

    use crate::{error::Error, Socket};

    fn recv(sock: &Socket, multipart: &mut Multipart) -> Poll<Multipart, Error> {
        loop {
            let msg = try_ready!(recv_msg(sock));
            let more = msg.get_more();

            multipart.push_back(msg);

            if !more {
                return Ok(Async::Ready(mem::replace(multipart, Multipart::new())));
            }
        }
    }

    fn recv_msg(sock: &Socket) -> Poll<Message, Error> {
        let mut msg = Message::new();

        match sock.recv_msg(&mut msg) {
            Ok(_) => Ok(Async::Ready(msg)),
            Err(zmq::Error::EAGAIN) => {
                debug!("ResponseFuture: EAGAIN");
                Ok(Async::NotReady)
            }
            Err(e) => {
                error!("Recv error: {}", e);
                Err(e.into())
            }
        }
    }

    pub(crate) fn poll(
        sock: &Socket,
        multipart: &mut Multipart,
    ) -> Poll<Multipart, Error> {
        let ready = Ready::readable();

        try_ready!(sock.poll_read_ready(ready));

        match recv(sock, multipart)? {
            Async::Ready(multipart) => {
                futures::task::current().notify();
                Ok(Async::Ready(multipart))
            }
            Async::NotReady => {
                sock.clear_ready(ready)?;
                Ok(Async::NotReady)
            }
        }
    }
}
