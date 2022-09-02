use std::{pin::Pin, time::Duration};

use anyhow::{Context, Result};
use clap::{arg, value_parser, AppSettings, ArgAction, ArgGroup, Command};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_serial::{DataBits, Parity, SerialStream, StopBits};

use crate::device::GpsDevice;

mod cat;
mod config;
mod proxy;
mod put;
mod server;

pub struct CmdData {
    verbose: bool,
    device: GpsDevice<DeviceType>,
}

pub enum DeviceType {
    Serial(SerialStream),
    Net(TcpStream),
}

impl AsyncWrite for DeviceType {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match *self.get_mut() {
            Self::Serial(ref mut x) => unsafe { Pin::new_unchecked(x).poll_write(cx, buf) },
            Self::Net(ref mut x) => unsafe { Pin::new_unchecked(x).poll_write(cx, buf) },
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match *self {
            Self::Serial(ref mut x) => unsafe { Pin::new_unchecked(x).poll_flush(cx) },
            Self::Net(ref mut x) => unsafe { Pin::new_unchecked(x).poll_flush(cx) },
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match *self {
            Self::Serial(ref mut x) => unsafe { Pin::new_unchecked(x).poll_shutdown(cx) },
            Self::Net(ref mut x) => unsafe { Pin::new_unchecked(x).poll_shutdown(cx) },
        }
    }
}

impl AsyncRead for DeviceType {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match *self {
            Self::Serial(ref mut x) => unsafe { Pin::new_unchecked(x).poll_read(cx, buf) },
            Self::Net(ref mut x) => unsafe { Pin::new_unchecked(x).poll_read(cx, buf) },
        }
    }
}

pub async fn run() -> Result<()> {
    let matches = Command::new("gps")
        .version("0.1")
        .arg(
            arg!(
            -v --verbose "Enable verbose output"
            )
            .action(ArgAction::SetTrue),
        )
        .arg(
            arg!(
                -s --serial <PATH> "Set the serial port"
            )
            .required(false),
        )
        .arg(
            arg!(
                -i --ip <ADDRESS> "The ip port to connect to"
            )
            .required(false),
        )
        .group(
            ArgGroup::new("device")
                .required(false)
                .args(&["ip", "serial"]),
        )
        .arg(
            arg!(
                -b --baud <BOUD> "Set the baud rate for the serial port"
            )
            .required(false)
            .requires("serial")
            .default_value("9600")
            .value_parser(value_parser!(u32)),
        )
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(config::subcmd())
        .subcommand(cat::subcmd())
        .subcommand(server::subcmd())
        .subcommand(put::subcmd())
        .subcommand(proxy::subcmd())
        .get_matches();

    let verbose = *matches.get_one::<bool>("verbose").unwrap();

    let mut device = if let Some(ip_addr) = matches.get_one::<String>("ip") {
        let stream = TcpStream::connect(ip_addr).await?;

        GpsDevice::new(DeviceType::Net(stream))
    } else {
        let port_path = matches
            .get_one::<String>("serial")
            .cloned()
            .unwrap_or_else(|| "/dev/ttyACM0".to_string());
        let port_baud = *matches.get_one::<u32>("baud").unwrap();
        let port = tokio_serial::new(port_path, port_baud)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .timeout(Duration::from_secs(1));

        let port = SerialStream::open(&port).context("failed to open serial port")?;

        GpsDevice::new(DeviceType::Serial(port))
    };

    device.read().await.ok();

    let mut data = CmdData { verbose, device };

    match matches.subcommand() {
        Some(("config", matches)) => config::cmd(&mut data, matches).await,
        Some(("cat", matches)) => cat::cmd(&mut data, matches).await,
        Some(("server", matches)) => server::cmd(&mut data, matches).await,
        Some(("put", matches)) => put::cmd(&mut data, matches).await,
        Some(("proxy", matches)) => proxy::cmd(&mut data, matches).await,
        _ => unreachable!(),
    }
}
