use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

use crate::http::{Request, Response};
use crate::middleware::{Middleware, MiddlewareChain, Next};
use crate::service::{ConnectionFlag, Service};

pub struct Server<E, S> {
    middlewares: MiddlewareChain<E>,
    services: Vec<Arc<dyn Service<E, S>>>,
}

impl<E, S> Server<E, S> {
    pub fn new(next: impl Next<E> + 'static) -> Self {
        Self {
            middlewares: MiddlewareChain::new(next),
            services: Vec::new(),
        }
    }

    pub fn middleware(mut self, middleware: impl Middleware<E> + 'static) -> Self {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    pub fn service(mut self, service: impl Service<E, S> + 'static) -> Self {
        self.services.push(Arc::new(service));
        self
    }

    pub fn leak(self) -> &'static Self {
        Box::leak(Box::new(self))
    }

    pub async fn handle_connection(
        &'static self,
        mut stream: S,
        address: SocketAddr,
    ) -> Result<(), E>
    where
        E: From<std::io::Error> + Send,
        S: AsyncReadExt + AsyncWriteExt + Unpin,
    {
        loop {
            let mut request = Request::from_buf_async(BufReader::new(&mut stream)).await?;
            let response = self.middlewares.call(&mut request).await?;
            stream.write_all(&response.to_bytes()).await?;
            stream.flush().await?;

            for service in self.services.iter() {
                let flag = service
                    .call(&request, &response, &address, &mut stream)
                    .await?;
                if let ConnectionFlag::Close = flag {
                    return Ok(());
                }
            }
        }
    }
}

impl<E, S> Default for Server<E, S>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    fn default() -> Self {
        Self::new(Response::new(404, "Not Found"))
            .middleware(crate::middleware::ContentLengthMiddleware)
            .service(crate::service::LoggerService)
    }
}
