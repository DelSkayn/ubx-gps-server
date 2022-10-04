use std::{net::SocketAddr, str::FromStr};

use anyhow::{anyhow, bail, Context as ErrorContext, Result};
use clap::{arg, Command};
use futures::{future, SinkExt, StreamExt};
use gps::{connection::Connection, msg::Rtcm, parse::ParseData, VecExt};
use hyper::{body::HttpBody, Body, Client, Request, Uri};
use log::{debug, trace, warn};
use tokio::net::TcpStream;

async fn run() -> Result<()> {
    let matches = Command::new("gps server")
        .version("0.1")
        .arg(
            arg!(
                -c --connect <ADDRESS> "Connect to an server."
            )
            .default_value("127.0.0.1:9165")
            .value_parser(SocketAddr::from_str)
            .required(false),
        )
        .arg(
            arg!(
                <ADDRESS> "The address of the NTRIP host"
            )
            .value_parser(Uri::from_str)
            .required(true),
        )
        .get_matches();

    let connect = matches.get_one::<SocketAddr>("connect").unwrap();
    let uri = matches.get_one::<Uri>("ADDRESS").unwrap();

    let client = Client::builder()
        .http09_responses(true)
        // Ntrip casters do not seem to http1 complient as header cases are not case
        // insensitive.
        .http1_title_case_headers(true)
        .build_http();

    let mut host = uri
        .host()
        .ok_or_else(|| anyhow!("uri missing host"))?
        .to_string();
    if let Some(port) = uri.port() {
        host = format!("{}:{}", host, port);
    }

    let request = Request::builder()
        .method("GET")
        .header("Host", host)
        .header("User-Agent", "NTRIP gps/0.1")
        .header("Accept", "*/*")
        .header("Ntrip-Version", "Ntrip/2.0")
        .uri(uri)
        .body(Body::empty())
        .context("failed to create request")?;

    debug!("sending ntrip request {:?}", request);

    let resp = client
        .request(request)
        .await
        .context("failed to send request")?;

    let ct_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|x| x.to_str().ok());
    if ct_type != Some("gnss/data") {
        bail!(
            "Ntrip caster did not return correct content type, found: {:?}",
            &ct_type
        );
    }

    let mut body = resp.into_body();

    let tcp = TcpStream::connect(connect)
        .await
        .context("could not create connection to server")?;

    let connection = Connection::new(tcp);

    let (mut sink, stream) = connection.split();

    //eat the incomming messages
    tokio::spawn(async {
        stream.skip_while(|_| future::ready(true)).count().await;
    });

    let mut buffer = Vec::new();
    loop {
        let data = body
            .data()
            .await
            .ok_or_else(|| anyhow!("ntrip caster disconnected"))?
            .context("reading error")?;
        buffer.extend_from_slice(&data);
        loop {
            let mut idx = 0;
            while buffer.len() > idx && buffer.len() > 2 && !Rtcm::contains_prefix(&buffer[idx..]) {
                idx += 1;
            }
            if idx != 0 {
                warn!("skipping {idx} bytes");
                buffer.shift(idx);
            }
            if let Some(x) = Rtcm::message_usage(&buffer) {
                trace!("writing message: {:?}", Rtcm::parse_read(&buffer));
                let mut b = buffer.split_off(x);
                std::mem::swap(&mut b, &mut buffer);
                sink.send(b).await?;
            } else {
                break;
            }
        }
    }
}

fn main() -> Result<()> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}
