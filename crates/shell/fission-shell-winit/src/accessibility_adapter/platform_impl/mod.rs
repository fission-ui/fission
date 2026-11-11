// Adapted from AccessKit's winit adapter, Apache-2.0, Copyright 2022 The AccessKit Authors.

pub use self::platform::Adapter;

#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
#[path = "unix.rs"]
mod platform;

#[cfg(target_os = "android")]
#[path = "android.rs"]
mod platform;

#[cfg(target_os = "ios")]
#[path = "ios.rs"]
mod platform;

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "android",
    target_os = "ios"
)))]
#[path = "null.rs"]
mod platform;
