use hyper::{Client, Request, Body, body::HttpBody};
use anyhow::{Result, Context, bail, anyhow};

use crate::{rtcm::RtcmFrame, parse};


pub struct Ntrip{
    body: Body,
    buffer: Vec<u8>,
}

impl Ntrip{
    pub async fn connect(addr: String) -> Result<Self>{
        let client = Client::new();

        let request = Request::builder()
            .method("GET")
            .uri(addr)
            .header("User-Agent","NTRIP gps/0.1")
            .header("Accept","*/*")
            .header("Ntrip-Version","Ntrip/2.0")
            .body(Body::empty())
            .context("failed to create request")?;

        let resp = client.request(request)
            .await
            .context("failed to send request")?;

        if resp.headers().get("Content-Type").and_then(|x| x.to_str().ok()) != Some("gnss/data"){
            bail!("Ntrip caster did not return correct content type");
        }

        let body = resp.into_body();

        Ok(Ntrip{
            body,
            buffer: Vec::new(),
        })
    }

    pub async fn resp(&mut self) -> Result<RtcmFrame<'static>>{
        loop{
            match RtcmFrame::from_bytes(&self.buffer){
                Ok((x,used)) => {
                    let x = x.into_owned();
                    let len = self.buffer.len();
                    self.buffer.copy_within(used..,0);
                    self.buffer.truncate(len - used);
                    return Ok(x);
                }
                Err(parse::Error::NotEnoughData) => {
                    let data = self.body.data().await.ok_or_else(|| anyhow!("ntrip caster disconnected"))??;
                    self.buffer.extend_from_slice(&data);
                }
                Err(parse::Error::InvalidHeader) => {
                    self.buffer.copy_within(1..,0);
                    self.buffer.pop();
                }
                Err(e) => bail!(e),
            }
        }
    }
}
