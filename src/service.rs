use std::net::SocketAddr;

use async_trait::async_trait;

use crate::http::{Request, Response};

pub enum ConnectionFlag {
    Continue,
    Close,
}

#[async_trait]
pub trait Service<E, S>: Send + Sync {
    async fn call(
        &self,
        request: &Request,
        response: &Response,
        address: &SocketAddr,
        stream: &mut S,
    ) -> Result<ConnectionFlag, E>;
}

pub struct LoggerService;

#[async_trait]
impl<E, S> Service<E, S> for LoggerService {
    async fn call(
        &self,
        request: &Request,
        response: &Response,
        address: &SocketAddr,
        _: &mut S,
    ) -> Result<ConnectionFlag, E> {
        use colored::Colorize;
        eprintln!(
            r#"{} - "{}" - {}"#,
            address,
            request.request_line().bright_cyan(),
            response
                .status_code
                .color(match response.status_code.to_string().chars().next() {
                    Some('1') => "cyan",
                    Some('2') => "green",
                    Some('3') => "yellow",
                    Some('4') => "red",
                    Some('5') => "purple",
                    _ => "normal",
                }),
        );
        Ok(ConnectionFlag::Continue)
    }
}
