use std::{
    collections::VecDeque,
    io::{stdin, stdout, Write},
    net::SocketAddr,
    str::FromStr,
};

use anyhow::{anyhow, Context as ErrorContext, Result};
use clap::{arg, Command};
use futures::StreamExt;
use gps::{connection::OutgoingConnection, msg::GpsMsg, parse::ParseData};
use termion::screen::AlternateScreen;

pub struct Writer {
    size: (u16, u16),
    cursor: (u16, u16),
    buffer: Vec<u8>,
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Writer {
    fn reset_size(&mut self) -> Result<()> {
        self.size = termion::terminal_size()?;
        Ok(())
    }

    fn clear(&mut self) {
        write!(&mut self.buffer, "{}", termion::cursor::Goto(1, 1)).unwrap();
        write!(&mut self.buffer, "{}", termion::clear::All).unwrap();
        self.cursor = (0, 0);
    }

    fn write_line(&mut self, line: &str) {
        let remaining = self.size.0 - self.cursor.0;
        if line.len() > remaining as usize {
            self.cursor.0 = self.size.1;
            write!(&mut self.buffer, "{}", &line[..(remaining as usize) - 3]).unwrap();
            write!(&mut self.buffer, "...").unwrap();
        } else {
            self.cursor.0 = self.cursor.0 + line.len() as u16;
            write!(&mut self.buffer, "{}", line).unwrap();
        }
    }

    fn goto(&mut self, pos: (u16, u16)) {
        self.cursor = pos;
        write!(
            &mut self.buffer,
            "{}",
            termion::cursor::Goto(1 + self.cursor.0, 1 + self.cursor.1)
        )
        .unwrap();
    }

    fn next_line(&mut self) {
        self.cursor.0 = 0;
        self.cursor.1 += 1;
        write!(
            &mut self.buffer,
            "{}",
            termion::cursor::Goto(1, 1 + self.cursor.1)
        )
        .unwrap();
    }

    fn flush(&mut self, w: &mut impl Write) -> Result<()> {
        w.write_all(&self.buffer)?;
        self.buffer.clear();
        Ok(())
    }
}

pub struct Info {
    last_itow: Option<u32>,
    error: Option<String>,
    messages: VecDeque<GpsMsg>,
    writer: Writer,
}

impl Info {
    pub fn new() -> Self {
        Info {
            last_itow: None,
            error: None,
            messages: VecDeque::new(),
            writer: Writer {
                size: (0, 0),
                cursor: (0, 0),
                buffer: Vec::new(),
            },
        }
    }

    pub fn redraw<W: Write>(&mut self, w: &mut W) -> Result<()> {
        self.writer.reset_size()?;
        self.writer.clear();

        let height = self.writer.size.1;
        let offset = height / 2;
        self.writer.goto((0, offset));
        write!(
            &mut self.writer,
            "{}",
            termion::color::Fg(termion::color::Green)
        )?;
        for m in self.messages.iter() {
            let msg = format!("{:?}", m);
            self.writer.write_line(&msg);
            if self.writer.cursor.1 >= self.writer.size.1 - 1 {
                break;
            }
            self.writer.next_line();
        }
        write!(
            &mut self.writer,
            "{}",
            termion::color::Fg(termion::color::Reset)
        )?;
        self.writer.flush(w)?;
        Ok(())
    }

    fn handle_msg(&mut self, msg: &GpsMsg) {
        match *msg {
            _ => {}
        }
    }

    pub fn push_message(&mut self, msg: GpsMsg) {
        self.handle_msg(&msg);
        self.messages.push_front(msg);
        if self.messages.len() > 100 {
            self.messages.pop_back();
        }
    }
}

async fn run() -> Result<()> {
    let matches = Command::new("gps monitor")
        .version("0.1")
        .arg(
            arg!(
                [ADDRESS] "The address to connect too"
            )
            .required(false)
            .default_value("127.0.0.1:9165")
            .value_parser(SocketAddr::from_str),
        )
        .get_matches();

    let address = matches.get_one::<SocketAddr>("ADDRESS").unwrap();
    let mut outgoing_connection = OutgoingConnection::new(Some(*address));

    let mut screen = AlternateScreen::from(stdout());

    let mut info = Info::new();

    while let Some(x) = outgoing_connection.next().await {
        match GpsMsg::parse_read(&x) {
            Ok((_, m)) => {
                info.push_message(m);
            }
            Err(e) => {
                info.error = Some(format!("parsing error: `{e}`"));
            }
        }
        info.redraw(&mut screen)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}
