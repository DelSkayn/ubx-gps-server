from gps_socket import GpsConnection
import time

conn = GpsConnection()

conn.send({"UbxPoll": {"Nav": "Clock"}})

while True:
    msg = conn.next()
    if msg is None:
        time.sleep(0.01)
    else:
        print(msg)
