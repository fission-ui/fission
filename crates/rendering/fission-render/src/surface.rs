use std::any::Any;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::capabilities::ColorFormat;

mod native_window_target;
pub use native_window_target::{NativeWindowTarget, NativeWindowTargetError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SurfaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

impl PhysicalSize {
    pub const ZERO: Self = Self {
        width: 0,
        height: 0,
    };

    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleFactor(f64);

impl ScaleFactor {
    pub const ONE: Self = Self(1.0);

    pub fn new(value: f64) -> Result<Self, InvalidScaleFactor> {
        if value.is_finite() && value > 0.0 {
            Ok(Self(value))
        } else {
            Err(InvalidScaleFactor(value))
        }
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidScaleFactor(pub f64);

impl fmt::Display for InvalidScaleFactor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "scale factor must be finite and greater than zero, got {}",
            self.0
        )
    }
}

impl std::error::Error for InvalidScaleFactor {}

impl Serialize for ScaleFactor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for ScaleFactor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceKind {
    NativeWindow,
    WebCanvas,
    Headless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadAffinity {
    Any,
    CreatingThread,
    MainThread,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceDescriptor {
    pub id: SurfaceId,
    pub kind: SurfaceKind,
    pub size: PhysicalSize,
    pub scale_factor: ScaleFactor,
    pub color_format: ColorFormat,
    pub thread_affinity: ThreadAffinity,
}

/// Backend-neutral view of a host-owned presentation target.
///
/// The host retains the concrete target for at least as long as an attached
/// session. A presenter may downcast `as_any` only inside its backend adapter;
/// dependency-specific surface types must not cross into frame, layout, or
/// widget code.
pub trait SurfaceTarget: fmt::Debug {
    fn descriptor(&self) -> &SurfaceDescriptor;
    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Detached,
    Attached,
    Suspended,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LossKind {
    Surface,
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recovery {
    Reattached,
    DeviceRecreated,
    SwitchedToSoftware,
    Unrecoverable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryPressure {
    Moderate,
    Critical,
}

/// Reusable lifecycle validator for backend sessions.
///
/// It does not perform rendering work; it prevents adapters from accepting an
/// invalid ordering such as presenting before attachment or resuming a live
/// session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLifecycle {
    state: SessionState,
}

impl Default for SessionLifecycle {
    fn default() -> Self {
        Self {
            state: SessionState::Detached,
        }
    }
}

impl SessionLifecycle {
    pub const fn state(self) -> SessionState {
        self.state
    }

    pub fn attach(&mut self) -> Result<(), LifecycleError> {
        self.transition("attach", &[SessionState::Detached], SessionState::Attached)
    }

    pub fn suspend(&mut self) -> Result<(), LifecycleError> {
        self.transition(
            "suspend",
            &[SessionState::Attached],
            SessionState::Suspended,
        )
    }

    pub fn resume(&mut self) -> Result<(), LifecycleError> {
        self.transition("resume", &[SessionState::Suspended], SessionState::Attached)
    }

    pub fn mark_lost(&mut self) -> Result<(), LifecycleError> {
        self.transition(
            "mark_lost",
            &[SessionState::Attached, SessionState::Suspended],
            SessionState::Lost,
        )
    }

    pub fn recover(&mut self) -> Result<(), LifecycleError> {
        self.transition("recover", &[SessionState::Lost], SessionState::Attached)
    }

    pub fn detach(&mut self) -> Result<(), LifecycleError> {
        self.transition(
            "detach",
            &[
                SessionState::Attached,
                SessionState::Suspended,
                SessionState::Lost,
            ],
            SessionState::Detached,
        )
    }

    pub fn require_attached(self, operation: &'static str) -> Result<(), LifecycleError> {
        if self.state == SessionState::Attached {
            Ok(())
        } else {
            Err(LifecycleError {
                operation,
                actual: self.state,
                expected: vec![SessionState::Attached],
            })
        }
    }

    /// Poison an ambiguously mutated session so no operation can continue to
    /// use a target whose backend state is no longer proven.
    pub(crate) fn fail_closed(&mut self) {
        self.state = SessionState::Lost;
    }

    fn transition(
        &mut self,
        operation: &'static str,
        expected: &[SessionState],
        next: SessionState,
    ) -> Result<(), LifecycleError> {
        if expected.contains(&self.state) {
            self.state = next;
            Ok(())
        } else {
            Err(LifecycleError {
                operation,
                actual: self.state,
                expected: expected.to_vec(),
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleError {
    pub operation: &'static str,
    pub actual: SessionState,
    pub expected: Vec<SessionState>,
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot {} while session is {:?}; expected one of {:?}",
            self.operation, self.actual, self.expected
        )
    }
}

impl std::error::Error for LifecycleError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_enforces_attach_suspend_resume_order() {
        let mut lifecycle = SessionLifecycle::default();

        assert!(lifecycle.resume().is_err());
        lifecycle.attach().unwrap();
        lifecycle.require_attached("render").unwrap();
        lifecycle.suspend().unwrap();
        assert!(lifecycle.require_attached("present").is_err());
        lifecycle.resume().unwrap();
        lifecycle.detach().unwrap();
        assert_eq!(lifecycle.state(), SessionState::Detached);
    }

    #[test]
    fn scale_factor_rejects_non_finite_and_non_positive_values() {
        assert!(ScaleFactor::new(0.0).is_err());
        assert!(ScaleFactor::new(f64::NAN).is_err());
        assert_eq!(ScaleFactor::new(2.0).unwrap().get(), 2.0);
        assert!(serde_json::from_str::<ScaleFactor>("0").is_err());
    }
}
