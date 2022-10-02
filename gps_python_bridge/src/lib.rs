use std::{io, net::SocketAddr, str::FromStr};

use futures::{
    channel::mpsc::{self, Receiver, Sender},
    future::{self, Either},
    SinkExt, StreamExt,
};
use gps::{connection::Connection, msg::GpsMsg, parse::ParseData};
use pyo3::{exceptions::PyException, prelude::*};
use tokio::net::TcpStream;

#[pyclass]
pub struct GpsConnection {
    send: Sender<GpsMsg>,
    recv: Receiver<Result<GpsMsg, io::Error>>,
}

impl GpsConnection {
    async fn socket_loop(
        address: SocketAddr,
        mut send: Sender<Result<GpsMsg, io::Error>>,
        mut recv: Receiver<GpsMsg>,
    ) {
        let tcp = match TcpStream::connect(address).await {
            Ok(x) => x,
            Err(e) => {
                send.send(Err(e)).await.ok();
                return;
            }
        };
        let mut connection = Connection::new(tcp);

        let mut buffer = Vec::new();

        loop {
            match future::select(connection.next(), recv.next()).await {
                Either::Left((Some(Ok(x)), _)) => {
                    if let Ok((_, msg)) =
                        GpsMsg::parse_read(&x).map_err(|e| println!("error parsing message: {e}"))
                    {
                        if let Err(e) = send.try_send(Ok(msg)) {
                            if e.is_disconnected() {
                                return;
                            }
                        }
                    }
                }
                Either::Left((Some(Err(e)), _)) => {
                    send.send(Err(e)).await.ok();
                }
                Either::Left((None, _)) => return,
                Either::Right((Some(x), _)) => {
                    buffer.clear();
                    x.parse_write(&mut buffer).unwrap();
                    if let Err(e) = connection.write_message(&buffer).await {
                        println!("connection error: {e}");
                    }
                }
                Either::Right((None, _)) => return,
            }
        }
    }
}

#[pymethods]
impl GpsConnection {
    #[new]
    #[args(address = "\"0.0.0.0:9165\"")]
    fn new(address: &str) -> PyResult<Self> {
        let addr = SocketAddr::from_str(&address)?;
        let (send_a, recv_a) = mpsc::channel(64);
        let (send_b, recv_b) = mpsc::channel(64);
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(GpsConnection::socket_loop(addr, send_a, recv_b));
        });

        Ok(GpsConnection {
            send: send_b,
            recv: recv_a,
        })
    }

    fn next(&mut self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        match self.recv.try_next() {
            Ok(Some(Ok(x))) => pythonize::pythonize(py, &x)
                .map(Some)
                .map_err(|x| PyException::new_err(format!("serialization error {x}"))),
            Ok(Some(Err(e))) => Err(PyException::new_err(format!("socket error {e}"))),
            Ok(None) => Err(PyException::new_err("gps socket quit")),
            Err(_) => return Ok(None),
        }
    }

    fn send(&mut self, object: &PyAny) -> PyResult<()> {
        let msg = pythonize::depythonize::<GpsMsg>(object)
            .map_err(|e| PyException::new_err(format!("serialization error {e}")))?;

        match self.send.try_send(msg) {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.is_disconnected() {
                    return Err(PyException::new_err("gps socket disconnected"));
                }
                Ok(())
            }
        }
    }
}

#[pymodule]
fn gps_socket(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<GpsConnection>()?;
    Ok(())
}
