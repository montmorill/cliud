use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Io error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Invalid opcode: {0}")]
    InvalidOpcode(u8),
    #[error("Bad protocol")]
    BadProtocol,
    #[error("Pong timeout")]
    PongTimeout,
}
pub type Result<T> = std::result::Result<T, Error>;

type Packet = (Opcode, Vec<u8>);

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Opcode {
    Continue = 0,
    Text = 1,
    Binary = 2,
    Close = 8,
    Ping = 9,
    Pong = 10,
}

impl TryFrom<u8> for Opcode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Opcode::Continue),
            1 => Ok(Opcode::Text),
            2 => Ok(Opcode::Binary),
            8 => Ok(Opcode::Close),
            9 => Ok(Opcode::Ping),
            10 => Ok(Opcode::Pong),
            _ => Err(Error::InvalidOpcode(value)),
        }
    }
}

async fn receive_packet(stream: &mut (impl AsyncReadExt + Unpin)) -> Result<Packet> {
    let mut mask = [0u8; 4];
    let mut data = Vec::new();
    let mut buf = Vec::new();
    let mut opcode = Opcode::Continue;
    loop {
        let head = stream.read_u16().await?;
        let finish = head & 0x8000 != 0;
        let code = ((head & 0x0f00) >> 8) as u8; // u4 actually
        let masked = head & 0x0080 != 0;
        let payload_len = (head & 0x007f) as u8; // u7 actually

        let code = Opcode::try_from(code)?;
        if code >= Opcode::Close && payload_len >= 0x7e {
            return Err(Error::BadProtocol);
        }
        match (&opcode, &code) {
            (Opcode::Continue, _) if code != Opcode::Continue => opcode = code,
            (_, Opcode::Continue) if opcode != Opcode::Continue => {}
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
    stream: &mut (impl AsyncWriteExt + Unpin),
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

type AsyncOutput<T = ()> = Pin<Box<dyn Send + Sync + Future<Output = T>>>;
pub struct WebSocket<'a, Stream: AsyncReadExt + AsyncWriteExt + Unpin> {
    stream: Stream,
    waiting_pong: bool,
    half_closed: bool,
    timeout: Duration,
    last_ping_time: Instant,
    on_message: Option<&'a (dyn Sync + Send + Fn(Vec<u8>) -> AsyncOutput)>,
    on_close: Option<&'a (dyn Sync + Send + Fn(Vec<u8>) -> AsyncOutput)>,
    on_pong: Option<&'a (dyn Sync + Send + Fn(Duration) -> AsyncOutput)>,
}

impl<'a, Stream: AsyncReadExt + AsyncWriteExt + Unpin> WebSocket<'a, Stream> {
    pub fn new(stream: Stream) -> Self {
        Self {
            stream,
            waiting_pong: true,
            half_closed: false,
            timeout: Duration::from_secs(5),
            last_ping_time: Instant::now(),
            on_message: None,
            on_close: None,
            on_pong: None,
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn on_message(mut self, f: &'a (impl Sync + Send + Fn(Vec<u8>) -> AsyncOutput)) -> Self {
        self.on_message = Some(f);
        self
    }

    pub fn on_close(mut self, f: &'a (impl Sync + Send + Fn(Vec<u8>) -> AsyncOutput)) -> Self {
        self.on_close = Some(f);
        self
    }

    pub fn on_pong(mut self, f: &'a (impl Sync + Send + Fn(Duration) -> AsyncOutput)) -> Self {
        self.on_pong = Some(f);
        self
    }

    async fn send_packet(&mut self, packet: Packet, mask: u32) -> Result<()> {
        send_packet(&mut self.stream, packet, mask).await
    }

    pub async fn send_text(&mut self, text: String) -> Result<()> {
        self.send_packet((Opcode::Text, text.into()), 0).await
    }

    pub async fn send_binary(&mut self, data: Vec<u8>) -> Result<()> {
        self.send_packet((Opcode::Binary, data), 0).await
    }

    pub async fn close(&mut self, reason: String) -> Result<()> {
        self.half_closed = true;
        self.send_packet((Opcode::Close, reason.into()), 0).await
    }

    async fn ping(&mut self) -> Result<()> {
        self.last_ping_time = Instant::now();
        self.send_packet((Opcode::Ping, Vec::new()), 0).await
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let packet = receive_packet(&mut self.stream);
            let (opcode, data) = match timeout(self.timeout, packet).await {
                Ok(packet) => packet?,
                Err(_) => {
                    if self.waiting_pong {
                        return Err(Error::PongTimeout);
                    }
                    self.ping().await?;
                    self.waiting_pong = true;
                    continue;
                }
            };
            match opcode {
                Opcode::Text | Opcode::Binary => {
                    if let Some(on_message) = self.on_message {
                        on_message(data).await;
                    }
                }
                Opcode::Close => {
                    if let Some(on_close) = self.on_close {
                        on_close(data.clone()).await;
                    }
                    if !self.half_closed {
                        self.send_packet((opcode, data), 0).await?;
                    }
                    return Ok(());
                }
                Opcode::Ping => self.send_packet((Opcode::Pong, data), 0).await?,
                Opcode::Pong => {
                    if let Some(on_pong) = self.on_pong {
                        on_pong(Instant::now() - self.last_ping_time).await;
                    }
                }

                _ => unreachable!(),
            }
        }
    }
}

pub async fn handle_websocket(stream: TcpStream, address: SocketAddr) -> Result<()> {
    WebSocket::new(stream)
        .on_message(&|msg| {
            Box::pin(async move {
                eprintln!("receive msg from {address}: {msg:?}");
            })
        })
        .run()
        .await
}
