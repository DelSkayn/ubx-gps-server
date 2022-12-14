
from socket import socket
import json
import sys
import os
import time

if len(sys.argv) < 3:
    print("Enter a adress");

addr_socket = sys.argv[1]
port_socket = sys.argv[2]

class GpsSocket:
    def __init__(self):
        self.socket = socket()
        self.bytes = b'';
        self.addr = None

    def connect(self, addr):
        self.addr = addr
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
            l = len(self.bytes)
            self.bytes += self.socket.recv(4096)
            if l == len(self.bytes):
                raise Exception("gps connection quit")

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
    ['Ubx'],
    #['Ubx','Nav','Svin'],
    #['Ubx','Mon','Rf','blocks','0','ant_status'],
    #['Ubx','Mon','Rf','blocks','0','ant_power'],
    #['Ubx','Mon','Rf','blocks','0','noise_per_ms'],
    #['Ubx','Mon','Rf','blocks','1','ant_status'],
    #['Ubx','Mon','Rf','blocks','1','ant_power'],
    #['Ubx','Mon','Rf','blocks','1','noise_per_ms'],
    #['Ubx','Nav','Pvt','flags','car_sol'],
    #['Ubx','Nav','Pvt','flags','diff_soln'],
    #['Ubx','Nav','Pvt','s_acc'],
    #['Ubx','Nav','Pvt','v_acc'],
    #['Ubx','Mon','Comms','blocks','3'],
    #['Ubx','Nav','HPPOSecef'],
    #['Ubx','Nav','HPPOSecef','p_acc'],
    ['Ubx','Nav','RelPosNed','i_tow'],
    #['Ubx','Nav','RelPosNed','flags'],
    #['Ubx','Nav','RelPosNed','rel_pos_n'],
    #['Ubx','Nav','RelPosNed','rel_pos_e'],
    #['Ubx','Nav','RelPosNed','acc_n'],
    #['Ubx','Nav','RelPosNed','acc_e'],
    #['Ubx','Rxm','Rtcm','msg_type'],
]

while True:
    msg = socket.next()
    stamp = time.time()

    for p,f in filter(msg,PATHS):
        print(stamp,p,":",f)
