use std::{net::Ipv4Addr, path::Path};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

use crate::error::{Error, Result};

use super::{encode, Control, Event, Request};

pub struct Session {
    writer: Writer,
    reader: Reader,
    pub pid: u32,
    pub started_at: u64,
}

pub struct Writer(OwnedWriteHalf);

pub struct Reader(tokio::io::Lines<BufReader<OwnedReadHalf>>);

impl Writer {
    pub async fn send(&mut self, request: &Request) -> Result<()> {
        self.0.write_all(encode(request)?.as_bytes()).await?;
        self.0.flush().await?;
        Ok(())
    }
}

impl Reader {
    pub async fn next(&mut self) -> Option<Event> {
        loop {
            let line = self.0.next_line().await.ok()??;
            if let Ok(event) = super::decode::<Event>(&line) {
                return Some(event);
            }
        }
    }
}

impl Session {
    pub fn split(self) -> (Writer, Reader) {
        (self.writer, self.reader)
    }
}

impl Session {
    pub async fn connect(control: &Control) -> Result<Self> {
        let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, control.port)).await?;
        let (reading, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reading).lines();

        writer
            .write_all(
                encode(&Request::Hello {
                    token: control.token.clone(),
                })?
                .as_bytes(),
            )
            .await?;
        writer.flush().await?;

        let greeting = reader
            .next_line()
            .await?
            .ok_or_else(|| Error::other("The server supervisor closed the connection."))?;
        match super::decode::<Event>(&greeting)? {
            Event::Ready { pid, started_at } => Ok(Session {
                writer: Writer(writer),
                reader: Reader(reader),
                pid,
                started_at,
            }),
            Event::Refused { reason } => Err(Error::other(format!(
                "The server supervisor refused the connection: {reason}"
            ))),
            other => Err(Error::other(format!(
                "The server supervisor said something unexpected: {other:?}"
            ))),
        }
    }
}

pub fn still_running(control: &Control) -> bool {
    crate::launch::identity::process_start(control.child_pid)
        .is_some_and(|started| started == control.child_started_at)
}

pub fn read_for(root: &Path, server_id: &str) -> Option<Control> {
    let path = super::control_path(root, server_id);
    let control = super::read_control(&path)?;
    if still_running(&control) {
        return Some(control);
    }
    super::forget_control(&path);
    None
}
