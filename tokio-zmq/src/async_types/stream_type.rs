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

use async_zmq_types::Multipart;
use futures::{task::Task, try_ready, Async, Poll};
use log::error;

use crate::{async_types::future_types::response, error::Error, Socket};

pub(crate) struct StreamType {
    multipart: Multipart,
}

impl StreamType {
    pub(crate) fn new() -> Self {
        StreamType {
            multipart: Multipart::new(),
        }
    }

    pub(crate) fn poll(
        &mut self,
        sock: &Socket,
        task: Option<&Task>,
    ) -> Poll<Option<Multipart>, Error> {
        let mpart = try_ready!(response::poll(&sock, &mut self.multipart, task));

        Ok(Async::Ready(Some(mpart)))
    }
}

impl Drop for StreamType {
    fn drop(&mut self) {
        if self.multipart.len() > 0 {
            error!(
                "DROPPING RECEIVED NON-EMPTY MULTIPART, {}",
                self.multipart.len()
            );
        }
    }
}
