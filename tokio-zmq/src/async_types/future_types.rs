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
    use zmq::{self, Message, DONTWAIT, SNDMORE};

    use crate::{async_types::EventedFile, error::Error};

    fn send(sock: &zmq::Socket, multipart: &mut Multipart) -> Poll<(), Error> {
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

    fn send_msg(sock: &zmq::Socket, msg: Message, last: bool) -> Result<Option<Message>, Error> {
        let flags = DONTWAIT | if last { 0 } else { SNDMORE };

        let msg_clone = Message::from(&*msg);

        match sock.send(msg, flags) {
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

    fn poll_write_ready(sock: &zmq::Socket, file: &EventedFile) -> Poll<(), Error> {
        let events = sock.get_events()?;

        if events & zmq::POLLOUT == zmq::POLLOUT {
            return Ok(Async::Ready(()));
        }

        file.clear_write_ready()?;
        Ok(Async::NotReady)
    }

    pub(crate) fn poll(
        sock: &zmq::Socket,
        file: &EventedFile,
        multipart: &mut Multipart,
    ) -> Poll<(), Error> {
        let _ = file.poll_write_ready()?;
        try_ready!(poll_write_ready(sock, file));

        match send(sock, multipart)? {
            Async::Ready(()) => {
                Ok(Async::Ready(()))
            }
            Async::NotReady => {
                file.clear_write_ready()?;
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
    use zmq::{self, Message, DONTWAIT};

    use crate::{async_types::EventedFile, error::Error};

    fn recv(sock: &zmq::Socket, multipart: &mut Multipart) -> Poll<Multipart, Error> {
        loop {
            let msg = try_ready!(recv_msg(sock));
            let more = msg.get_more();

            multipart.push_back(msg);

            if !more {
                return Ok(Async::Ready(mem::replace(multipart, Multipart::new())));
            }
        }
    }

    fn recv_msg(sock: &zmq::Socket) -> Poll<Message, Error> {
        let mut msg = Message::new();

        match sock.recv(&mut msg, DONTWAIT) {
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

    fn poll_read_ready(sock: &zmq::Socket, file: &EventedFile) -> Poll<(), Error> {
        let events = sock.get_events()?;

        if events & zmq::POLLIN == zmq::POLLIN {
            return Ok(Async::Ready(()));
        }

        file.clear_read_ready(Ready::readable())?;
        Ok(Async::NotReady)
    }

    pub(crate) fn poll(
        sock: &zmq::Socket,
        file: &EventedFile,
        multipart: &mut Multipart,
    ) -> Poll<Multipart, Error> {
        let ready = Ready::readable();

        let _ = file.poll_read_ready(ready)?;
        try_ready!(poll_read_ready(sock, file));

        match recv(sock, multipart)? {
            Async::Ready(multipart) => {
                futures::task::current().notify();
                Ok(Async::Ready(multipart))
            }
            Async::NotReady => {
                file.clear_read_ready(ready)?;
                Ok(Async::NotReady)
            }
        }
    }
}
