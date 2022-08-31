
from socket import socket
import json
import sys

if len(sys.argv) < 3:
    print("Enter a adress");

addr_socket = sys.argv[1]
port_socket = sys.argv[2]

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
socket.connect((addr_socket,int(port_socket)))

def lookup(dict,path):
    if isinstance(dict,list):
        next = dict[int(path[0])]
        if len(path) > 1:
            return lookup(next,path[1:])
        else:
            return next

    next = dict.get(path[0])
    if next is None or len(path[1:]) == 0:
        return next
    else:
        return lookup(next,path[1:])

def filter(dict,paths):
    for p in paths:
        res = lookup(dict,p)
        if res is not None:
            yield '.'.join(p), res


PATHS = [
    #['Ubx','Nav','Svin'],
    ['Ubx','Mon','Rf','blocks','0','ant_status'],
    ['Ubx','Mon','Rf','blocks','0','ant_power'],
    ['Ubx','Mon','Rf','blocks','0','noise_per_ms'],
    ['Ubx','Mon','Rf','blocks','1','ant_status'],
    ['Ubx','Mon','Rf','blocks','1','ant_power'],
    ['Ubx','Mon','Rf','blocks','1','noise_per_ms'],
    ['Ubx','Nav','Pvt','flags','car_sol'],
    ['Ubx','Nav','Pvt','flags','diff_soln'],
    ['Ubx','Nav','Pvt','s_acc'],
    ['Ubx','Nav','Pvt','v_acc'],
    #['Ubx','Nav','HPPOSecef'],
    ['Ubx','Nav','HPPOSecef','p_acc'],
    ['Ubx','Nav','RelPosNed'],
    ['Ubx','Rxm','Rtcm','msg_type'],
]

while True:
    msg = socket.next()

    sig = lookup(msg, ['Ubx','Nav','Sig','blocks'])
    if sig is not None:
        print(sum([b['cno'] for b in sig]) / len(sig))
    #print(msg)
    for p,f in filter(msg,PATHS):
        print(p,":",f)
