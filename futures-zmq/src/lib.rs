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

#[macro_use]
extern crate async_zmq_derive;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

pub mod async_types;
pub mod error;
mod poll_thread;
pub mod prelude;
mod socket;

pub use async_zmq_types::Multipart;

pub use self::{
    error::Error,
    poll_thread::{RecvFuture, SendFuture, SendRecvFuture, Session},
    socket::{
        types::{Dealer, Pair, Pub, Pull, Push, Rep, Req, Router, Sub, Xpub, Xsub},
        Socket,
    },
};

lazy_static! {
    pub static ref SESSION: Session = Session::new();
}
