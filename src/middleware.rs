use std::sync::Arc;

use async_trait::async_trait;

use crate::http::{Request, Response};

#[async_trait]
pub trait Next<E>: Send + Sync {
    async fn call(&self, request: &Request) -> Result<Response, E>;
}

#[async_trait]
impl<E> Next<E> for Response {
    async fn call(&self, _request: &Request) -> Result<Response, E> {
        Ok(self.clone())
    }
}

#[async_trait]
pub trait Middleware<E>: Send + Sync {
    async fn call(&self, request: &Request, next: &dyn Next<E>) -> Result<Response, E>;
}

pub struct MiddlewareNext<E> {
    middleware: Arc<dyn Middleware<E>>,
    next: Arc<dyn Next<E>>,
}

#[async_trait]
impl<E: Send> Next<E> for MiddlewareNext<E> {
    async fn call(&self, request: &Request) -> Result<Response, E> {
        self.middleware.call(request, &*self.next).await
    }
}

pub struct MiddlewareChain<E> {
    middlewares: Vec<Arc<dyn Middleware<E>>>,
    next: Arc<dyn Next<E>>,
}

impl<E> MiddlewareChain<E> {
    pub fn new(next: impl Next<E> + 'static) -> Self {
        Self {
            middlewares: Vec::new(),
            next: Arc::new(next),
        }
    }

    pub fn push(&mut self, middleware: Arc<dyn Middleware<E>>) {
        self.middlewares.push(middleware);
    }
}

#[async_trait]
impl<E: Send + 'static> Next<E> for MiddlewareChain<E> {
    async fn call(&self, request: &Request) -> Result<Response, E> {
        let mut next: Arc<dyn Next<E>> = Arc::clone(&self.next);
        for middleware in self.middlewares.iter().rev() {
            let middleware = Arc::clone(middleware);
            let middleware_next = MiddlewareNext { middleware, next };
            next = Arc::new(middleware_next);
        }
        next.call(request).await
    }
}

pub struct ContentLengthMiddleware;

#[async_trait]
impl<E> Middleware<E> for ContentLengthMiddleware {
    async fn call(&self, request: &Request, next: &dyn Next<E>) -> Result<Response, E> {
        let response = next.call(request).await?;
        let length = response.body.len();
        if length != 0 {
            Ok(response.header("Content-Length", length))
        } else {
            Ok(response)
        }
    }
}
