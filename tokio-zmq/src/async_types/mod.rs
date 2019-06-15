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

//! This module contains the code that makes Tokio ZMQ Asynchronous. There's the `future` module,
//! which defines Request and Response futures for ZeroMQ Sockets, the `stream` module, which
//! defines receiving data from a socket as an asychronous stream, and the `sink` module, which
//! defines sending data to a socket as an asychronous sink.

use tokio_reactor::PollEvented;

use crate::file::ZmqFile;

pub mod future;
mod future_types;
pub mod sink;
pub mod sink_stream;
mod sink_type;
pub mod stream;
mod stream_type;

pub use self::{
    future::{MultipartRequest, MultipartResponse},
    sink::MultipartSink,
    sink_stream::MultipartSinkStream,
    stream::{ControlledStream, EndingStream, MultipartStream, TimeoutStream},
};

pub type EventedFile = PollEvented<ZmqFile>;
