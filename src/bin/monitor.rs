use std::{
    collections::VecDeque,
    io::{stdout, Write},
    net::SocketAddr,
    str::FromStr,
};

use anyhow::Result;
use clap::{arg, Command};
use futures::StreamExt;
use gps::{
    connection::OutgoingConnection,
    msg::{
        ubx::{
            mon::{CommBlock, Mon},
            nav::{Nav, Pvt, RelPosNed},
            rxm::Rxm,
        },
        GpsMsg, Ubx,
    },
    parse::ParseData,
};
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
    comms: Vec<CommBlock>,
    acked_rtcm: Vec<u16>,
    prev_acked_rtcm: Vec<u16>,
    pvt: Option<Pvt>,
    relposned: Option<RelPosNed>,
    writer: Writer,
}

impl Info {
    pub fn new() -> Self {
        Info {
            last_itow: None,
            error: None,
            messages: VecDeque::new(),
            comms: Vec::new(),
            pvt: None,
            relposned: None,
            acked_rtcm: Vec::new(),
            prev_acked_rtcm: Vec::new(),
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

        for (idx, b) in self.comms.iter().enumerate() {
            let msg = format!(
                "port {idx}({:>3}): rx/tx {:>3}%/{:>3}% errors: {:>4}, skipped: {:>6}",
                b.port_id, b.rx_usage, b.tx_usage, b.overrun_errs, b.skipped
            );
            self.writer.write_line(&msg);
            self.writer.next_line();
        }
        if !self.comms.is_empty() {
            self.writer.next_line();
        }

        if !self.prev_acked_rtcm.is_empty() {
            self.writer.write_line("RXM RTCM: ");
            for x in self.prev_acked_rtcm.iter().copied() {
                self.writer.write_line(&format!("{x} "));
            }
            self.writer.next_line();
            self.writer.next_line();
        }

        if let Some(x) = self.pvt.as_ref() {
            self.writer.write_line("PVT:");
            self.writer.next_line();
            self.writer.write_line("    ");
            let line = format!(
                "fix `{:?}` diff_active `{:?}` car_sol `{:?}`",
                x.fix_type, x.flags.diff_soln, x.flags.car_sol
            );
            self.writer.write_line(&line);
            self.writer.next_line();
            self.writer.write_line("    ");
            let line = format!(
                "acc h/v {:>6.3}/{:<6.3}, ",
                x.h_acc as f32 / 1000.0,
                x.v_acc as f32 / 1000.0
            );
            self.writer.write_line(&line);
            self.writer.next_line();
            self.writer.next_line();
        }

        if let Some(x) = self.relposned.as_ref() {
            self.writer.write_line("RelPosNed:");
            self.writer.next_line();
            self.writer.write_line("    ");
            let line = format!("fix `{:?}`", x.flags,);
            self.writer.write_line(&line);
            self.writer.next_line();
            self.writer.write_line("    ");
            let line = format!(
                "acc n/e/d {:.3}/{:.3}/{:.3} len {:.3} ",
                x.acc_n as f64 / 1000.0,
                x.acc_e as f64 / 1000.0,
                x.acc_d as f64 / 1000.0,
                x.acc_length as f64 / 1000.0,
            );
            self.writer.write_line(&line);
            self.writer.next_line();
            self.writer.write_line("    ");
            let line = format!(
                "pos n/e/d {:.3}/{:.3}/{:.3} len {:.3} ",
                x.rel_pos_n as f64 / 1000.0,
                x.rel_pos_e as f64 / 1000.0,
                x.rel_pos_d as f64 / 1000.0,
                x.rel_pos_length as f64 / 1000.0,
            );
            self.writer.write_line(&line);
            self.writer.next_line();
            self.writer.next_line();
        }

        if let Some(x) = self.error.as_ref() {
            write!(
                &mut self.writer,
                "{}",
                termion::color::Fg(termion::color::Red)
            )?;
            self.writer.write_line("ERROR: ");
            self.writer.write_line(&x);
            write!(
                &mut self.writer,
                "{}",
                termion::color::Fg(termion::color::Reset)
            )?;
            self.writer.next_line();
            self.writer.next_line();
        }

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

    fn handle_itow(&mut self, itow: u32) {
        if self.last_itow == Some(itow) {
            return;
        }
        self.last_itow = Some(itow);
        self.prev_acked_rtcm.clear();
        std::mem::swap(&mut self.prev_acked_rtcm, &mut self.acked_rtcm);
        self.error.take();
    }

    fn handle_msg(&mut self, msg: &GpsMsg) {
        match *msg {
            GpsMsg::Ubx(Ubx::Rxm(Rxm::Rtcm(ref x))) => {
                self.acked_rtcm.push(x.msg_type);
            }
            GpsMsg::Ubx(Ubx::Nav(Nav::Eoe(ref x))) => {
                self.handle_itow(x.i_tow);
            }
            GpsMsg::Ubx(Ubx::Nav(Nav::Pvt(ref x))) => {
                self.handle_itow(x.i_tow);
                self.pvt = Some(x.clone())
            }
            GpsMsg::Ubx(Ubx::Nav(Nav::RelPosNed(ref x))) => {
                self.handle_itow(x.i_tow);
                self.relposned = Some(x.clone())
            }
            GpsMsg::Ubx(Ubx::Mon(Mon::Comms(ref comms))) => {
                self.comms.clear();
                for b in comms.blocks.iter().cloned() {
                    self.comms.push(b);
                }
            }
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
