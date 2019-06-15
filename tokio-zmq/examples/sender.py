#!/usr/bin/env python

import zmq

ctx = zmq.Context()
sock = ctx.socket(zmq.DEALER)
sock.set_hwm(8192 * 2)
sock.connect("ipc:///tmp/lost-send")

count = 5000

for _ in range(count):
    sock.send_multipart([b"msg"])

counter = 0
for _ in range(count):
    sock.recv_multipart()
    counter += 1
    if counter % 100 == 0:
        print(counter)
