#![allow(async_fn_in_trait)]

use std::ops::{Deref, DerefMut};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time;

use crate::http::{Request, Response};
use crate::middleware::{Middleware, Next};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Invalid Opcode: {0}")]
    InvalidOpcode(u8),
    #[error("Bad Protocol")]
    BadProtocol,
    #[error("Pong Timeout")]
    PongTimeout,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub type Packet = (Opcode, Vec<u8>);

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Opcode {
    Continuation = 0,
    Text = 1,
    Binary = 2,
    Close = 8,
    Ping = 9,
    Pong = 10,
}

impl Opcode {
    pub fn is_control(self) -> bool {
        self >= Self::Close
    }
}

impl TryFrom<u8> for Opcode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Opcode::Continuation),
            1 => Ok(Opcode::Text),
            2 => Ok(Opcode::Binary),
            8 => Ok(Opcode::Close),
            9 => Ok(Opcode::Ping),
            10 => Ok(Opcode::Pong),
            _ => Err(Error::InvalidOpcode(value)),
        }
    }
}

async fn receive_packet(
    mut stream: impl DerefMut<Target = impl AsyncReadExt + Unpin>,
) -> Result<Packet> {
    let mut mask = [0u8; 4];
    let mut data = Vec::new();
    let mut buf = Vec::new();
    let mut opcode = Opcode::Continuation;
    loop {
        let head = stream.read_u16().await?;
        let finish = head & 0x8000 != 0;
        let code = ((head & 0x0f00) >> 8) as u8; // u4 actually
        let masked = head & 0x0080 != 0;
        let payload_len = (head & 0x007f) as u8; // u7 actually

        let code = Opcode::try_from(code)?;
        if code.is_control() && (payload_len >= 0x7e || !finish) {
            return Err(Error::BadProtocol);
        }
        match (opcode == Opcode::Continuation, code == Opcode::Continuation) {
            (false, true) => {}
            (true, false) => opcode = code,
            _ => return Err(Error::BadProtocol),
        }

        let payload_len = match payload_len {
            0x7e => stream.read_u16().await? as usize,
            0x7f => stream.read_u64().await? as usize,
            _ => payload_len as usize,
        };
        if masked {
            stream.read_exact(&mut mask).await?;
        }
        buf.resize(payload_len, 0);
        stream.read_exact(&mut buf).await?;
        if masked {
            for (index, value) in buf.iter_mut().enumerate() {
                *value ^= mask[index % 4];
            }
        }
        data.extend(&buf);

        if finish {
            return Ok((opcode, data));
        }
    }
}

async fn send_packet(
    mut stream: impl DerefMut<Target = impl AsyncWriteExt + Unpin>,
    (opcode, mut data): Packet,
    mask: u32,
) -> Result<()> {
    // suppose there is only one frame...
    let finish = true;
    let masked = mask != 0;
    let payload_len = match data.len() {
        len @ ..=0x7d => len as u8, // < 0x7e
        ..=0xffff => 0x7e,          // <= 0xffff
        _ => 0x7f,
    };
    let head0 = (finish as u8) << 7 | opcode as u8;
    let head1 = (masked as u8) << 7 | payload_len;
    stream.write_all(&[head0, head1]).await?;
    if data.len() > 0x7e {
        if data.len() <= 0xffff {
            stream.write_u16(data.len() as u16).await?;
        } else {
            stream.write_u64(data.len() as u64).await?;
        }
    }
    if masked {
        stream.write_u32(mask).await?;
        let mask = [
            (mask >> 24) as u8,
            (mask >> 16) as u8,
            (mask >> 8) as u8,
            mask as u8,
        ];
        for (index, value) in data.iter_mut().enumerate() {
            *value ^= mask[index % 4];
        }
    }
    stream.write_all(&data).await?;
    stream.flush().await?;

    Ok(())
}

#[derive(Debug)]
pub struct WebSocketState {
    pub mask: u32,
    pub timeout: Duration,
    waiting_pong: bool,
    half_closed: bool,
    last_ping_time: Instant,
}

impl Default for WebSocketState {
    fn default() -> Self {
        Self {
            mask: 0,
            timeout: Duration::from_secs(5),
            waiting_pong: false,
            half_closed: false,
            last_ping_time: Instant::now(),
        }
    }
}

impl WebSocketState {
    pub fn mask(mut self, mask: u32) -> Self {
        self.mask = mask;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

pub trait WebSocket: Send + Sync {
    type Stream: AsyncReadExt + AsyncWriteExt + Unpin;

    async fn stream_mut(&self) -> impl DerefMut<Target = Self::Stream>;

    async fn state(&self) -> impl Deref<Target = WebSocketState>;

    async fn state_mut(&self) -> impl DerefMut<Target = WebSocketState>;

    async fn on_message(&mut self, message: Vec<u8>) -> Result<()> {
        let _ = message;
        Ok(())
    }

    async fn on_close(&mut self, reason: Vec<u8>) -> Result<()> {
        let _ = reason;
        Ok(())
    }

    async fn on_pong(&mut self, delay: Duration) -> Result<()> {
        let _ = delay;
        Ok(())
    }
}

pub trait WebSocketExt: WebSocket {
    async fn send_packet(&mut self, packet: Packet) -> Result<()> {
        send_packet(self.stream_mut().await, packet, self.state().await.mask).await
    }

    async fn send_text(&mut self, text: String) -> Result<()> {
        self.send_packet((Opcode::Text, text.into())).await
    }

    async fn send_binary(&mut self, data: Vec<u8>) -> Result<()> {
        self.send_packet((Opcode::Binary, data)).await
    }

    async fn send_close(&mut self, reason: String) -> Result<()> {
        self.state_mut().await.half_closed = true;
        self.send_packet((Opcode::Close, reason.into())).await
    }

    async fn send_ping(&mut self) -> Result<()> {
        self.state_mut().await.last_ping_time = Instant::now();
        self.state_mut().await.waiting_pong = true;
        self.send_packet((Opcode::Ping, Vec::new())).await
    }

    async fn run(&mut self) -> Result<()> {
        loop {
            let timeout = self.state().await.timeout;
            let future = receive_packet(self.stream_mut().await);
            let (opcode, data) = match time::timeout(timeout, future).await {
                Ok(packet) => packet?,
                Err(_) => {
                    if self.state().await.waiting_pong {
                        return Err(Error::PongTimeout);
                    }
                    self.send_ping().await?;
                    continue;
                }
            };
            self.state_mut().await.waiting_pong = false;
            match opcode {
                Opcode::Text | Opcode::Binary => self.on_message(data).await?,
                Opcode::Close => {
                    if !self.state().await.half_closed {
                        self.on_close(data.clone()).await?;
                        self.send_packet((opcode, data)).await?;
                    }
                    return Ok(());
                }
                Opcode::Ping => self.send_packet((Opcode::Pong, data)).await?,
                Opcode::Pong => {
                    let delay = Instant::now() - self.state().await.last_ping_time;
                    self.on_pong(delay).await?;
                }
                Opcode::Continuation => unreachable!(),
            }
        }
    }
}

impl<T: WebSocket> WebSocketExt for T {}

pub struct WebSocketHandshakeMiddleware;

#[async_trait]
impl<E> Middleware<E> for WebSocketHandshakeMiddleware {
    async fn call(&self, request: &mut Request, next: &dyn Next<E>) -> Result<Response, E> {
        if let Some(upgrade) = request.headers.get("Upgrade")
            && upgrade == "websocket"
        {
            if let Some(key) = request.headers.get("Sec-WebSocket-Key") {
                use base64::prelude::*;
                use sha1_smol::Sha1;

                let concated = [key, "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"].concat();
                let hashed = Sha1::from(concated).digest().bytes();
                let encoded = BASE64_STANDARD.encode(hashed);

                Ok(Response::new(101, "Switching Protocols")
                    .header("Upgrade", "websocket")
                    .header("Connection", "Upgrade")
                    .header("Sec-Websocket-Accept", encoded)
                    .header("Sec-Websocket-Version", "13"))
            } else {
                Ok(Response::new(400, "Bad Request"))
            }
        } else {
            next.call(request).await
        }
    }
}
