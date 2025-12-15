use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, AsyncWriteExt as _, BufReader};

use crate::http::{self, Request, Response};
use crate::middleware::{Middleware, MiddlewareChain, Next};
use crate::service::{ConnectionFlag, Service};

pub struct Server<E, S> {
    middlewares: MiddlewareChain<E>,
    services: Vec<Arc<dyn Service<E, S>>>,
}

impl<E, S> Server<E, S> {
    #[inline]
    pub fn new(next: impl Next<E> + 'static) -> Self {
        Self {
            middlewares: MiddlewareChain::new(next),
            services: Vec::new(),
        }
    }

    #[inline]
    pub fn with_middleware(mut self, middleware: impl Middleware<E> + 'static) -> Self {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    #[inline]
    pub fn with_service(mut self, service: impl Service<E, S> + 'static) -> Self {
        self.services.push(Arc::new(service));
        self
    }

    #[inline]
    pub fn leak(self) -> &'static Self {
        Box::leak(Box::new(self))
    }

    #[inline]
    pub async fn handle_request(
        &'static self,
        stream: impl AsyncBufRead + Unpin,
    ) -> Result<(Option<Request>, Response), E>
    where
        E: From<std::io::Error> + Send,
    {
        match Request::try_from_buf_async(stream).await {
            Ok(request) => {
                let response = self.middlewares.call(&request).await?;
                Ok((Some(request), response))
            }
            Err(http::Error::BadRequestLine(line)) => {
                let response = Response::new(400, "Bad Request").plain(format!("Bad Request Line: {line}"));
                Ok((None, response))
            }
            Err(http::Error::BadContentLength(length)) => {
                let response = Response::new(400, "Bad Request").plain(format!("Bad Content Length: {length}"));
                Ok((None, response))
            }
            Err(http::Error::ContentLengthRequired) => {
                let response = Response::new(411, "Length Required").plain("Content Length Required");
                Ok((None, response))
            }
            Err(http::Error::IO(e)) => Err(e.into()),
        }
    }

    #[inline]
    pub async fn handle_connection(&'static self, mut stream: S, address: SocketAddr) -> Result<(), E>
    where
        E: From<std::io::Error> + Send,
        S: AsyncRead + AsyncWrite + Unpin,
    {
        loop {
            let (request, response) = self.handle_request(BufReader::new(&mut stream)).await?;
            stream.write_all(&response.to_bytes()).await?;
            stream.flush().await?;

            if let Some(request) = request {
                for service in self.services.iter() {
                    let flag = service.call(&request, &response, &address, &mut stream).await?;
                    if let ConnectionFlag::Close = flag {
                        return Ok(());
                    }
                }
            }
        }
    }
}

impl<E, S> Default for Server<E, S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn default() -> Self {
        Self::new(Response::new(404, "Not Found"))
            .with_middleware(crate::middleware::ContentLengthMiddleware)
            .with_service(crate::service::LoggerService)
    }
}
