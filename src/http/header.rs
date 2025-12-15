use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result};

use smol_str::{SmolStr, ToSmolStr};

#[derive(Debug, Clone, Default)]
pub struct HeaderMap {
    inner: HashMap<SmolStr, SmolStr>,
}

impl HeaderMap {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn insert(&mut self, key: impl ToSmolStr, value: impl ToSmolStr) {
        self.inner.insert(key.to_smolstr(), value.to_smolstr());
    }

    #[inline]
    pub fn get(&self, key: impl ToSmolStr) -> Option<&SmolStr> {
        self.inner.get(&key.to_smolstr())
    }

    #[inline]
    pub fn remove(&mut self, key: impl ToSmolStr) {
        self.inner.remove(&key.to_smolstr());
    }
}

impl Display for HeaderMap {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        #[expect(clippy::iter_over_hash_type, reason = "headers order is not important")]
        for (key, value) in &self.inner {
            write!(f, "{}: {}\r\n", key, value)?;
        }
        Ok(())
    }
}
