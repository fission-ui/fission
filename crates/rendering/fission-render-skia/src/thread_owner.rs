use std::fmt;
use std::thread::{self, ThreadId};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThreadOwner {
    id: ThreadId,
}

impl ThreadOwner {
    pub(crate) fn current() -> Self {
        Self {
            id: thread::current().id(),
        }
    }

    pub(crate) fn check(self) -> Result<(), WrongThread> {
        let actual = thread::current().id();
        if actual == self.id {
            Ok(())
        } else {
            Err(WrongThread {
                expected: self.id,
                actual,
            })
        }
    }
}

impl fmt::Debug for ThreadOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadOwner")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WrongThread {
    expected: ThreadId,
    actual: ThreadId,
}

impl fmt::Display for WrongThread {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Skia backend operation ran on thread {:?}; owning thread is {:?}",
            self.actual, self.expected
        )
    }
}

impl std::error::Error for WrongThread {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_rejects_a_different_thread() {
        let owner = ThreadOwner::current();

        assert!(owner.check().is_ok());
        assert!(thread::spawn(move || owner.check())
            .join()
            .unwrap()
            .is_err());
    }
}
