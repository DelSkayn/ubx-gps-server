
from socket import socket
import json

class GpsSocket:
    def __init__(self):
        self.socket = socket()
        self.bytes = b'';

    def connect(self, addr):
        self.socket.connect(addr)

    def parse(self):
        if len(self.bytes) < 4:
            return None

        length = int.from_bytes(self.bytes[:4],"little")
        if len(self.bytes) < length + 4:
            return None

        self.bytes = self.bytes[4:]
        str = self.bytes[:length]
        self.bytes = self.bytes[length:]
        return str.decode("utf-8")

    def next(self):
        while True:
            msg = self.parse()
            if msg is not None:
                return json.loads(msg)
            self.bytes += self.socket.recv(4096)

socket = GpsSocket()
socket.connect(("127.0.0.1",9165))

while True:
    print(socket.next())


