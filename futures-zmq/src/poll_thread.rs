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

#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, RawSocket};

use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Read, Write},
    marker::PhantomData,
    mem::transmute,
    net::{TcpListener, TcpStream},
    os::raw::c_void,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
};

use async_zmq_types::Multipart;
use futures::{
    executor::{self, Notify},
    sync::oneshot,
    Async, Future, Poll,
};
use libc::c_short;
use zmq::{poll, Message, PollEvents, PollItem, Socket, DONTWAIT, POLLIN, POLLOUT, SNDMORE};

use crate::error::Error;

enum Request {
    Init(Socket, oneshot::Sender<SockId>),
    SendMessage(usize, Multipart, usize, oneshot::Sender<Response>),
    ReceiveMessage(usize, oneshot::Sender<Response>),
    DropSocket(usize),
    Done,
}

pub(crate) trait DuplicateSock {
    fn dup(&self) -> Self;
}

pub struct SockId(usize, Arc<Mutex<SockIdInner>>);

impl SockId {
    fn new(id: usize, tx: Sender) -> Self {
        SockId(id, Arc::new(Mutex::new(SockIdInner(id, tx))))
    }
}

struct SockIdInner(usize, Sender);

impl DuplicateSock for SockId {
    fn dup(&self) -> Self {
        SockId(self.0, self.1.clone())
    }
}

impl Drop for SockIdInner {
    fn drop(&mut self) {
        trace!("Dropping {}", self.0);
        let _ = self.1.send(Request::DropSocket(self.0));
    }
}

enum Response {
    Sent,
    Received(Multipart),
    Full(Multipart),
    Error(Error),
}

enum PollKind {
    SendMsg,
    RecvMsg,
    SendRecv,
    UNUSED,
}

impl PollKind {
    fn as_events(&self) -> PollEvents {
        match *self {
            PollKind::SendMsg => POLLOUT,
            PollKind::RecvMsg => POLLIN,
            PollKind::SendRecv => POLLIN | POLLOUT,
            _ => 0,
        }
    }

    fn is_read(&self) -> bool {
        self.as_events() & POLLIN == POLLIN
    }

    fn is_write(&self) -> bool {
        self.as_events() & POLLOUT == POLLOUT
    }

    fn read(&mut self) {
        match *self {
            PollKind::SendMsg => *self = PollKind::SendRecv,
            _ => *self = PollKind::RecvMsg,
        }
    }

    fn clear_read(&mut self) {
        match *self {
            PollKind::SendRecv | PollKind::SendMsg => *self = PollKind::SendMsg,
            _ => *self = PollKind::UNUSED,
        }
    }

    fn write(&mut self) {
        match *self {
            PollKind::RecvMsg => *self = PollKind::SendRecv,
            _ => *self = PollKind::SendMsg,
        }
    }

    fn clear_write(&mut self) {
        match *self {
            PollKind::SendRecv | PollKind::RecvMsg => *self = PollKind::RecvMsg,
            _ => *self = PollKind::UNUSED,
        }
    }
}

struct Channel {
    ready: AtomicBool,
    tx: TcpStream,
    rx: TcpStream,
}

impl Channel {
    fn swap_false(&self) -> bool {
        self.ready.swap(false, Ordering::SeqCst)
    }

    fn swap_true(&self) -> bool {
        self.ready.swap(true, Ordering::SeqCst)
    }

    fn notify(&self) {
        if !self.swap_true() {
            let write_res = (&self.tx).write(&[1]);

            drop(write_res);
        }
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> RawFd {
        self.rx.as_raw_fd()
    }

    #[cfg(windows)]
    fn as_raw_fd(&self) -> RawSocket {
        self.rx.as_raw_socket()
    }
}

#[derive(Clone)]
struct Sender {
    tx: mpsc::Sender<Request>,
    channel: Arc<Channel>,
}

impl Sender {
    fn send(&self, request: Request) {
        let _ = self.tx.send(request);
        self.channel.notify();
    }
}

struct Receiver {
    rx: mpsc::Receiver<Request>,
    channel: Arc<Channel>,
}

impl Receiver {
    fn try_recv(&self) -> Option<Request> {
        self.rx.try_recv().ok()
    }

    /// Returns whether there are messages to look at
    fn drain(&self) -> bool {
        loop {
            match (&self.channel.rx).read(&mut [0; 32]) {
                Ok(_) => {}
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("I/O error: {}", e),
            }
        }

        return self.channel.swap_false();
    }
}

#[derive(Clone)]
pub struct Session {
    inner: Arc<InnerSession>,
}

impl Session {
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let conn1 = TcpStream::connect(&addr).unwrap();
        let conn2 = listener.accept().unwrap().0;

        drop(listener);

        conn1.set_nonblocking(true).unwrap();
        conn2.set_nonblocking(true).unwrap();

        let channel = Arc::new(Channel {
            ready: AtomicBool::new(false),
            tx: conn1,
            rx: conn2,
        });

        let (tx, rx) = mpsc::channel();

        let tx = Sender {
            tx: tx.clone(),
            channel: channel.clone(),
        };
        let rx = Receiver {
            rx: rx,
            channel: channel,
        };

        let tx2 = tx.clone();

        thread::spawn(move || {
            PollThread::new(tx2, rx).run();
        });

        Session {
            inner: InnerSession::init(tx),
        }
    }

    pub fn send(&self, id: &SockId, msg: Multipart, buffer_size: usize) -> SendFuture {
        let (tx, rx) = oneshot::channel();

        self.inner
            .send(Request::SendMessage(id.0, msg, buffer_size, tx));

        SendFuture { rx }
    }

    pub fn recv(&self, id: &SockId) -> RecvFuture {
        let (tx, rx) = oneshot::channel();

        self.inner.send(Request::ReceiveMessage(id.0, tx));

        RecvFuture { rx }
    }

    pub fn init(&self, sock: Socket) -> InitFuture {
        let (tx, rx) = oneshot::channel();

        self.inner.send(Request::Init(sock, tx));

        InitFuture { rx }
    }
}

pub struct SendFuture {
    rx: oneshot::Receiver<Response>,
}

impl Future for SendFuture {
    type Item = Option<Multipart>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.rx.poll()? {
            Async::Ready(res) => match res {
                Response::Sent => Ok(Async::Ready(None)),
                Response::Full(msg) => Ok(Async::Ready(Some(msg))),
                Response::Error(e) => Err(e),
                _ => panic!("Response kind was not sent"),
            },
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}

pub struct RecvFuture {
    rx: oneshot::Receiver<Response>,
}

impl Future for RecvFuture {
    type Item = Multipart;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.rx.poll()? {
            Async::Ready(res) => match res {
                Response::Received(msg) => Ok(Async::Ready(msg)),
                Response::Error(e) => Err(e),
                _ => panic!("Response kind was not received"),
            },
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}

pub struct InitFuture {
    rx: oneshot::Receiver<SockId>,
}

impl Future for InitFuture {
    type Item = SockId;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        Ok(self.rx.poll()?)
    }
}

struct InnerSession {
    tx: Mutex<Sender>,
}

impl InnerSession {
    fn init(tx: Sender) -> Arc<Self> {
        Arc::new(InnerSession { tx: Mutex::new(tx) })
    }

    fn send(&self, request: Request) {
        self.tx.lock().unwrap().clone().send(request);
    }
}

impl Drop for InnerSession {
    fn drop(&mut self) {
        self.tx.lock().unwrap().clone().send(Request::Done);
    }
}

#[derive(Clone)]
struct NotifyCanceled {
    channel: Arc<Channel>,
}

impl NotifyCanceled {
    fn new(channel: Arc<Channel>) -> Self {
        NotifyCanceled { channel }
    }
}

impl Notify for NotifyCanceled {
    fn notify(&self, _id: usize) {
        self.channel.notify();
    }
}

struct CheckCanceled<'a> {
    sender: &'a mut oneshot::Sender<Response>,
}

impl<'a> Future for CheckCanceled<'a> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        self.sender.poll_cancel()
    }
}

struct Pollable {
    sock: Socket,
    kind: PollKind,
    msg: VecDeque<Multipart>,
    pending_recv_msg: VecDeque<Multipart>,
    send_responder: Option<oneshot::Sender<Response>>,
    recv_responder: Option<oneshot::Sender<Response>>,
}

impl Pollable {
    fn new(sock: Socket) -> Self {
        Pollable {
            sock,
            kind: PollKind::UNUSED,
            msg: VecDeque::new(),
            pending_recv_msg: VecDeque::new(),
            send_responder: None,
            recv_responder: None,
        }
    }

    fn as_poll_item(&self) -> PollItem {
        self.sock.as_poll_item(self.kind.as_events())
    }

    fn is_readable(&self, poll_item: &PollItem) -> bool {
        self.kind.is_read() && poll_item.is_readable()
    }

    fn is_writable(&self, poll_item: &PollItem) -> bool {
        self.kind.is_write() && poll_item.is_writable()
    }

    fn read(&mut self) {
        self.kind.read();
    }

    fn clear_read(&mut self) {
        self.kind.clear_read();
    }

    fn write(&mut self) {
        self.kind.write();
    }

    fn clear_write(&mut self) {
        self.kind.clear_write();
    }

    fn message(&mut self, msg: Multipart, buffer_size: usize) -> Option<Multipart> {
        if self.msg.len() < buffer_size {
            self.msg.push_back(msg);
            None
        } else {
            Some(msg)
        }
    }

    fn send_responder(&mut self, r: oneshot::Sender<Response>) {
        self.send_responder = Some(r);
    }

    fn recv_responder(&mut self, r: oneshot::Sender<Response>) {
        self.recv_responder = Some(r);
    }

    fn recv_msg(&mut self) {
        'multiparts: loop {
            let mut multipart = Multipart::new();

            'messages: loop {
                match self.sock.recv_msg(DONTWAIT) {
                    Ok(msg) => {
                        let get_more = msg.get_more();
                        multipart.push_back(msg);

                        if get_more {
                            continue 'messages;
                        }
                        trace!("Received msg");
                        self.clear_read();
                        if let Some(responder) = self.recv_responder.take() {
                            if let Err(_) = responder.send(Response::Received(multipart)) {
                                error!("Error responding with received message");
                            }
                        } else {
                            self.pending_recv_msg.push_back(multipart);
                        }
                        continue 'multiparts;
                    }
                    Err(e) => match e {
                        zmq::Error::EAGAIN => {
                            warn!("EAGAIN while receiving");
                            self.clear_read();
                            break 'multiparts;
                        }
                        zmq::Error::EFSM => {
                            trace!("Tried to read after read should be done");
                            self.clear_read();
                            break 'multiparts;
                        }
                        e => {
                            error!("Error receiving message");
                            self.clear_read();
                            if let Some(responder) = self.recv_responder.take() {
                                if let Err(_) = responder.send(Response::Error(e.into())) {
                                    error!("Error responding with error");
                                }
                            } else {
                                error!("Error while receiving, {}, {}", e, e.to_raw());
                            }
                            break 'multiparts;
                        }
                    },
                }
            }
        }
    }

    fn send_msg(&mut self) {
        trace!("send_msg");
        loop {
            trace!("loop");
            if let Some(mut multipart) = self.msg.pop_front() {
                trace!("Got multipart");
                if let Some(msg) = multipart.pop_front() {
                    trace!("Got message to send");
                    let flags = DONTWAIT | if multipart.is_empty() { 0 } else { SNDMORE };

                    let msg_clone_res = Message::from_slice(&msg);

                    match self.sock.send_msg(msg, flags) {
                        Ok(_) => {
                            trace!("Sent message");
                            if !multipart.is_empty() {
                                self.msg.push_front(multipart);
                                trace!("Multipart not empty, continuing");
                                continue;
                            }
                            if !self.msg.is_empty() {
                                trace!("msg not empty, continuing");
                                continue;
                            }

                            self.clear_write();
                            if let Err(_) = self.send_responder.take().unwrap().send(Response::Sent)
                            {
                                error!("Error responding with sent");
                            }
                            break;
                        }
                        Err(e) => match e {
                            zmq::Error::EAGAIN => {
                                warn!("EAGAIN while sending");
                                match msg_clone_res {
                                    Ok(msg) => {
                                        multipart.push_front(msg);
                                        self.msg.push_front(multipart);
                                    }
                                    Err(e) => {
                                        self.clear_write();
                                        if let Err(_) = self
                                            .send_responder
                                            .take()
                                            .unwrap()
                                            .send(Response::Error(e.into()))
                                        {
                                            error!("Error responding with error");
                                        }
                                    }
                                }
                                break;
                            }
                            e => {
                                self.clear_write();
                                error!("Error sending message");
                                if let Err(_) = self
                                    .send_responder
                                    .take()
                                    .unwrap()
                                    .send(Response::Error(e.into()))
                                {
                                    error!("Error responding with error");
                                }
                                break;
                            }
                        },
                    }
                }
            } else {
                self.clear_write();
                break;
            }
        }
    }
}

#[repr(C)]
pub struct MyPollItem<'a> {
    socket: *mut c_void,
    fd: zmq_sys::RawFd,
    events: c_short,
    revents: c_short,
    marker: PhantomData<&'a Socket>,
}

impl<'a> MyPollItem<'a> {
    fn from_fd(fd: zmq_sys::RawFd, events: PollEvents) -> Self {
        MyPollItem {
            socket: ptr::null_mut(),
            fd,
            events,
            revents: 0,
            marker: PhantomData,
        }
    }
}

enum Action {
    Snd(usize),
    Rcv(usize),
}

struct PollThread {
    next_sock_id: usize,
    tx: Sender,
    rx: Receiver,
    should_stop: bool,
    to_action: Vec<Action>,
    notify: Arc<NotifyCanceled>,
    sockets: BTreeMap<usize, Pollable>,
    channel: Arc<Channel>,
}

impl PollThread {
    fn new(tx: Sender, rx: Receiver) -> Self {
        let channel = rx.channel.clone();

        PollThread {
            next_sock_id: 0,
            tx,
            rx,
            should_stop: false,
            to_action: Vec::new(),
            notify: Arc::new(NotifyCanceled::new(channel.clone())),
            sockets: BTreeMap::new(),
            channel,
        }
    }

    fn run(&mut self) {
        loop {
            self.turn();
        }
    }

    fn try_recv(&mut self) {
        if self.rx.drain() {
            trace!("new messages to handle");
            while let Some(msg) = self.rx.try_recv() {
                self.handle_request(msg);

                if self.should_stop {
                    break;
                }
            }
        }
    }

    fn handle_request(&mut self, request: Request) {
        match request {
            Request::Init(sock, responder) => {
                let id = self.next_sock_id;

                self.sockets.insert(id, Pollable::new(sock));
                if let Err(_) = responder.send(SockId::new(id, self.tx.clone())) {
                    error!("Error responding with init socket");
                }

                self.next_sock_id += 1;
            }
            Request::SendMessage(id, message, buffer_size, responder) => {
                trace!("Handling send");
                self.sockets.get_mut(&id).map(|pollable| {
                    if let Some(msg) = pollable.message(message, buffer_size) {
                        trace!("Buffer full");
                        if let Err(_) = responder.send(Response::Full(msg)) {
                            error!("Error notifying of full buffer");
                        }
                        return;
                    }
                    pollable.write();
                    pollable.send_responder(responder);
                });
            }
            Request::ReceiveMessage(id, responder) => {
                trace!("Handling recv");
                self.sockets.get_mut(&id).map(|pollable| {
                    if let Some(multipart) = pollable.pending_recv_msg.pop_front() {
                        trace!("responding with buffered data");
                        if let Err(_) = responder.send(Response::Received(multipart)) {
                            error!("Error responding with buffered data");
                        }
                        return;
                    }
                    pollable.recv_responder(responder);
                    pollable.read();
                });
            }
            Request::DropSocket(id) => {
                self.sockets.remove(&id);
            }
            Request::Done => {
                trace!("Handling done");
                self.should_stop = true;
            }
        }
    }

    fn check_responder(
        notify: &Arc<NotifyCanceled>,
        sender: &mut oneshot::Sender<Response>,
    ) -> bool {
        let mut cancel_check = executor::spawn(CheckCanceled { sender });

        if let Ok(Async::Ready(())) = cancel_check.poll_future_notify(notify, 0) {
            true
        } else {
            false
        }
    }

    fn drop_inactive(&mut self) {
        for ref mut pollable in self.sockets.values_mut() {
            if let Some(mut responder) = pollable.recv_responder.take() {
                let to_clear = Self::check_responder(&self.notify, &mut responder);

                if !to_clear {
                    pollable.recv_responder(responder);
                }
            }

            if let Some(mut responder) = pollable.send_responder.take() {
                let to_clear = Self::check_responder(&self.notify, &mut responder);

                if !to_clear {
                    pollable.send_responder(responder);
                }
            }
        }
    }

    fn poll(&mut self) {
        self.to_action.truncate(0);

        let (ids, mut poll_items): (Vec<_>, Vec<_>) = self
            .sockets
            .iter()
            .map(|(id, pollable)| (id, pollable.as_poll_item()))
            .unzip();

        let io_item = MyPollItem::from_fd(self.channel.as_raw_fd(), POLLIN);

        let io_item: PollItem = unsafe { transmute(io_item) };

        poll_items.push(io_item);

        let num_signalled = match poll(&mut poll_items, -1) {
            Ok(num) => num,
            Err(e) => {
                error!("Error in poll, {}", e);
                return;
            }
        };

        let mut count = 0;
        if num_signalled > 0 && poll_items[poll_items.len() - 1].is_readable() {
            count += 1;
        }

        for (id, item) in ids.into_iter().zip(poll_items) {
            // Prioritize outbound messages over inbound messages
            if self
                .sockets
                .get(&id)
                .map(|p| p.is_writable(&item))
                .unwrap_or(false)
            {
                trace!("{} is writable", id);
                self.to_action.push(Action::Snd(id));

                count += 1;
            } else if self
                .sockets
                .get(&id)
                .map(|p| p.is_readable(&item))
                .unwrap_or(false)
            {
                trace!("{} is readable", id);
                self.to_action.push(Action::Rcv(id));

                count += 1;
            }

            if count >= num_signalled {
                break;
            }
        }

        for action in self.to_action.drain(..).rev() {
            match action {
                Action::Rcv(id) => {
                    self.sockets
                        .get_mut(&id)
                        .map(|pollable| pollable.recv_msg());
                }
                Action::Snd(id) => {
                    self.sockets
                        .get_mut(&id)
                        .map(|pollable| pollable.send_msg());
                }
            }
        }
    }

    fn turn(&mut self) {
        self.drop_inactive();
        self.try_recv();
        self.poll();
    }
}
