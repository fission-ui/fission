use std::thread::{self, ThreadId};

use crate::{Error, ErrorKind, Result};

#[derive(Debug)]
pub(crate) struct ThreadAffinity {
    owner: ThreadId,
}

impl ThreadAffinity {
    pub(crate) fn current() -> Self {
        Self {
            owner: thread::current().id(),
        }
    }

    pub(crate) fn ensure_owner(&self, operation: &str) -> Result<()> {
        if self.owner == thread::current().id() {
            Ok(())
        } else {
            Err(Error::local(
                ErrorKind::WrongThread,
                operation,
                "the native handle was used from a thread other than its owner",
            ))
        }
    }

    pub(crate) fn is_owner(&self) -> bool {
        self.owner == thread::current().id()
    }
}
