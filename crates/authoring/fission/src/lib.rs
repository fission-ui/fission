//! # Fission
//!
//! A cross-platform, GPU-accelerated UI framework for Rust.
//!
//! This crate re-exports all Fission sub-crates so applications only need
//! a single dependency:
//!
//! ```toml
//! [dependencies]
//! fission = { path = "...", default-features = false, features = ["desktop"] }
//! ```
//!
//! Then use via:
//! ```rust,ignore
//! use fission::prelude::*;           // Common widget + action types
//! use fission::core::*;              // Low-level runtime/action APIs
//! use fission::widgets::*;           // Authoring widgets (Modal, Popover, etc.)
//! use fission::ir::*;                // Intermediate representation
//! use fission::theme::*;             // Theming
//! use fission::icons::material::*;   // Material icons
//! use fission::shell::DesktopApp;    // Desktop shell
//! use fission::text_engine::*;       // Rope-backed text buffer
//! ```

extern crate self as fission;

// ── Sub-crate re-exports ─────────────────────────────────────────────────

/// Core runtime, widgets, actions, reducers, effects.
pub mod core {
    pub use fission_core::*;
}

/// Intermediate representation (IR) — the node graph between widgets and layout.
pub mod ir {
    pub use fission_ir::*;
}

/// Layout engine — constraint-based layout with Box, Flex, Grid, Scroll, etc.
pub mod layout {
    pub use fission_layout::*;
}

/// Theming — design tokens, component themes, dark/light mode.
pub mod theme {
    pub use fission_theme::*;
}

/// Internationalisation — locale registry, string lookups.
pub mod i18n {
    pub use fission_i18n::*;
}

/// Text editing engine — rope-backed buffers, line indexes, and edit history.
pub mod text_engine {
    pub use fission_text_engine::*;
}

/// Authoring widgets — Modal, Popover, Tooltip, Menu, Combobox, SplitView, etc.
pub mod widgets {
    pub use fission_widgets::*;
}

/// Chart widgets and data-visualization primitives.
#[cfg(feature = "charts")]
pub mod charts {
    pub use fission_charts::*;
}

/// 3D scene and embed primitives.
#[cfg(feature = "three-d")]
pub mod three_d {
    pub use fission_3d::*;
}

/// Derive and attribute macros — `#[fission_action]`, `#[fission_reducer]`, and friends.
pub mod macros {
    pub use fission_core::{reduce, reduce_with, with_reducer};
    pub use fission_macros::*;
}

/// Material Design icons.
pub mod icons {
    pub use fission_icons::*;
}

/// Platform shells — desktop, mobile, and web wrappers over the shared runtime.
pub mod shell {
    #[cfg(all(
        any(feature = "desktop", feature = "platform-shells"),
        not(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))
    ))]
    pub use fission_shell_desktop::*;
    #[cfg(all(
        any(
            feature = "android",
            feature = "ios",
            feature = "mobile",
            feature = "platform-shells"
        ),
        any(target_os = "android", target_os = "ios")
    ))]
    pub use fission_shell_mobile::*;
    #[cfg(feature = "site")]
    pub use fission_shell_site::*;
    #[cfg(feature = "terminal-shell")]
    pub use fission_shell_terminal::*;
    #[cfg(all(
        any(feature = "web", feature = "platform-shells"),
        target_arch = "wasm32"
    ))]
    pub use fission_shell_web::*;
}

/// Static site shell APIs.
#[cfg(feature = "site")]
pub mod site {
    pub use fission_shell_site::*;
}

/// Terminal shell APIs.
#[cfg(feature = "terminal-shell")]
pub mod terminal {
    pub use fission_shell_terminal::*;
}

/// Rendering primitives — DisplayList, DisplayOp, TextStyle, Color.
pub mod render {
    pub use fission_render::*;
}

/// Diagnostics system — structured logging, performance tracing.
pub mod diagnostics {
    pub use fission_diagnostics::*;
}

/// Serialization traits and derives used by Fission action macros.
pub use serde;

/// Test driver — LiveTestClient, TestCommand, TestResponse.
#[cfg(feature = "test-driver")]
pub mod test_driver {
    pub use fission_test_driver::*;
}

// ── Flat re-exports for convenience ──────────────────────────────────────

// Core widget types (Button, Text, Container, Row, Column, etc.)
pub use fission_core::ui::*;

// Core action/state types
pub use fission_core::{
    Action, ActionEnvelope, ActionId, ActionScopeId, AnimationPropertyId, AnimationRequest,
    AnimationStartValue, AppState, AuthenticateBiometricCapability, BiometricAuthenticateRequest,
    BiometricAuthenticateResult, BiometricAvailability, BiometricEffects, BiometricError,
    BiometricKind, BiometricStrength, BuildCtx, CancelAllNotificationsCapability,
    CancelBiometricAuthenticationCapability, CancelNotificationCapability,
    CancelNotificationRequest, DeepLink, DeepLinkConfig, DeepLinkReceived, DeepLinkSource,
    EasingFunction, EmulateNfcTagCapability, FlexDirection, GetBiometricAvailabilityCapability,
    GetNfcAvailabilityCapability, GetNotificationSettingsCapability, Handler, NfcAvailability,
    NfcEffects, NfcEmulationRequest, NfcError, NfcRecord, NfcRecordTypeNameFormat, NfcScanRequest,
    NfcSessionReceipt, NfcTag, NfcTagDiscovered, NfcTechnology, NfcWriteRequest, NodeBuilder,
    NotificationActionButton, NotificationError, NotificationId, NotificationPermission,
    NotificationPermissionRequest, NotificationReceipt, NotificationRequest, NotificationResponse,
    NotificationResponseReceived, NotificationSchedule, NotificationSettings, NotificationSound,
    Op, PortalLayer, PushPlatform, PushRegistration, PushRegistrationRequest, ReducerContext,
    RegisterPushNotificationsCapability, RequestNotificationPermissionCapability,
    ScanNfcTagCapability, ScheduleNotificationCapability, Selector, SetBadgeCountCapability,
    SetBadgeCountRequest, ShowNotificationCapability, UnregisterPushNotificationsCapability, View,
    Widget, WidgetNodeId, WriteNfcTagCapability, AUTHENTICATE_BIOMETRIC, CANCEL_ALL_NOTIFICATIONS,
    CANCEL_BIOMETRIC_AUTHENTICATION, CANCEL_NFC_SESSION, CANCEL_NOTIFICATION, EMULATE_NFC_TAG,
    GET_BIOMETRIC_AVAILABILITY, GET_NFC_AVAILABILITY, GET_NOTIFICATION_SETTINGS,
    REGISTER_PUSH_NOTIFICATIONS, REQUEST_NOTIFICATION_PERMISSION, SCAN_NFC_TAG,
    SCHEDULE_NOTIFICATION, SET_BADGE_COUNT, SHOW_NOTIFICATION, UNREGISTER_PUSH_NOTIFICATIONS,
    WRITE_NFC_TAG,
};
pub use fission_core::{
    AdjustVolumeLevelCapability, GetVolumeLevelCapability, SetVolumeLevelCapability,
    VolumeAdjustDirection, VolumeAdjustRequest, VolumeEffects, VolumeError, VolumeLevel,
    VolumeSetRequest, VolumeStream, ADJUST_VOLUME_LEVEL, GET_VOLUME_LEVEL, SET_VOLUME_LEVEL,
};
pub use fission_core::{
    AudioSampleFormat, CancelMicrophoneCaptureCapability, CaptureMicrophoneAudioCapability,
    GetMicrophoneAvailabilityCapability, MicrophoneAvailability, MicrophoneCapture,
    MicrophoneCaptureRequest, MicrophoneDevice, MicrophoneEffects, MicrophoneError,
    MicrophonePermission, MicrophonePermissionRequest, RequestMicrophonePermissionCapability,
    CANCEL_MICROPHONE_CAPTURE, CAPTURE_MICROPHONE_AUDIO, GET_MICROPHONE_AVAILABILITY,
    REQUEST_MICROPHONE_PERMISSION,
};
pub use fission_core::{
    AuthenticatePasskeyCapability, CancelPasskeyOperationCapability,
    GetPasskeyAvailabilityCapability, PasskeyAlgorithm, PasskeyAttestationConveyance,
    PasskeyAuthenticationRequest, PasskeyAuthenticationResult, PasskeyAuthenticatorAttachment,
    PasskeyAuthenticatorSelection, PasskeyAvailability, PasskeyCredentialDescriptor,
    PasskeyEffects, PasskeyError, PasskeyMediation, PasskeyRegistrationRequest,
    PasskeyRegistrationResult, PasskeyRelyingParty, PasskeyResidentKeyRequirement,
    PasskeyTransport, PasskeyUser, PasskeyUserVerification, RegisterPasskeyCapability,
    AUTHENTICATE_PASSKEY, CANCEL_PASSKEY_OPERATION, GET_PASSKEY_AVAILABILITY, REGISTER_PASSKEY,
};
pub use fission_core::{
    BarcodeFormat, BarcodeImageDecodeRequest, BarcodePoint, BarcodeScanRequest, BarcodeScanResult,
    BarcodeScanResults, BarcodeScannerEffects, BarcodeScannerError, CancelBarcodeScanCapability,
    DecodeBarcodeImageCapability, ScanBarcodeCapability, CANCEL_BARCODE_SCAN, DECODE_BARCODE_IMAGE,
    SCAN_BARCODE,
};
pub use fission_core::{
    BluetoothAdvertiseReceipt, BluetoothAdvertiseRequest, BluetoothAvailability,
    BluetoothConnectRequest, BluetoothConnection, BluetoothDevice, BluetoothDisconnectRequest,
    BluetoothEffects, BluetoothError, BluetoothMode, BluetoothPermission,
    BluetoothPermissionRequest, BluetoothReadRequest, BluetoothReadResult, BluetoothScanRequest,
    BluetoothScanResult, BluetoothStopAdvertiseRequest, BluetoothWriteRequest,
    ConnectBluetoothDeviceCapability, DisconnectBluetoothDeviceCapability,
    GetBluetoothAvailabilityCapability, ReadBluetoothCharacteristicCapability,
    RequestBluetoothPermissionCapability, ScanBluetoothDevicesCapability,
    StartBluetoothAdvertisingCapability, StopBluetoothAdvertisingCapability,
    WriteBluetoothCharacteristicCapability, CONNECT_BLUETOOTH_DEVICE, DISCONNECT_BLUETOOTH_DEVICE,
    GET_BLUETOOTH_AVAILABILITY, READ_BLUETOOTH_CHARACTERISTIC, REQUEST_BLUETOOTH_PERMISSION,
    SCAN_BLUETOOTH_DEVICES, START_BLUETOOTH_ADVERTISING, STOP_BLUETOOTH_ADVERTISING,
    WRITE_BLUETOOTH_CHARACTERISTIC,
};
pub use fission_core::{
    CameraAvailability, CameraCapture, CameraCaptureRequest, CameraDevice, CameraEffects,
    CameraError, CameraFacing, CameraFlashMode, CameraFlashlightRequest, CameraImageFormat,
    CameraPermission, CameraPermissionRequest, CameraResolution, CancelCameraCaptureCapability,
    CapturePhotoCapability, GetCameraAvailabilityCapability, RequestCameraPermissionCapability,
    SetCameraFlashlightCapability, CANCEL_CAMERA_CAPTURE, CAPTURE_PHOTO, GET_CAMERA_AVAILABILITY,
    REQUEST_CAMERA_PERMISSION, SET_CAMERA_FLASHLIGHT,
};
pub use fission_core::{
    ClearClipboardCapability, ClipboardContent, ClipboardEffects, ClipboardError, ClipboardItem,
    ClipboardText, ClipboardWriteTextRequest, ReadClipboardContentCapability,
    ReadClipboardTextCapability, WriteClipboardContentCapability, WriteClipboardTextCapability,
    CLEAR_CLIPBOARD, READ_CLIPBOARD_CONTENT, READ_CLIPBOARD_TEXT, WRITE_CLIPBOARD_CONTENT,
    WRITE_CLIPBOARD_TEXT,
};
pub use fission_core::{
    ConnectWifiNetworkCapability, DisconnectWifiNetworkCapability, GetWifiAvailabilityCapability,
    RequestWifiPermissionCapability, ScanWifiNetworksCapability, WifiAvailability,
    WifiConnectRequest, WifiConnection, WifiDisconnectRequest, WifiEffects, WifiError, WifiNetwork,
    WifiPermission, WifiPermissionRequest, WifiScanRequest, WifiScanResult, WifiSecurity,
    CONNECT_WIFI_NETWORK, DISCONNECT_WIFI_NETWORK, GET_WIFI_AVAILABILITY, REQUEST_WIFI_PERMISSION,
    SCAN_WIFI_NETWORKS,
};
pub use fission_core::{
    GeolocationEffects, GeolocationError, GeolocationPermission, GeolocationPermissionRequest,
    GeolocationPosition, GeolocationPositionRequest, GetCurrentPositionCapability,
    GetGeolocationPermissionCapability, RequestGeolocationPermissionCapability,
    GET_CURRENT_POSITION, GET_GEOLOCATION_PERMISSION, REQUEST_GEOLOCATION_PERMISSION,
};
pub use fission_core::{
    HapticEffects, HapticError, HapticImpactCapability, HapticImpactRequest, HapticImpactStyle,
    HapticNotificationCapability, HapticNotificationKind, HapticNotificationRequest,
    HapticPatternCapability, HapticPatternRequest, HapticPatternStep, HapticSelectionCapability,
    HAPTIC_IMPACT, HAPTIC_NOTIFICATION, HAPTIC_PATTERN, HAPTIC_SELECTION,
};

// Core event types
pub use fission_core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
pub use fission_core::{reduce, reduce_with, with_reducer};

// Core env types
pub use fission_core::env::Env;

// IR op types (Color, LayoutOp, PaintOp, etc.)
pub use fission_ir::op;
pub use fission_ir::NodeId;

// Layout types
pub use fission_layout::{LayoutPoint, LayoutRect, LayoutSize, LayoutUnit};

// Authoring widgets (HStack, VStack, Spacer, Icon, etc.)
pub use fission_widgets::{HStack, Icon, Spacer, VStack};

// Platform shells
#[cfg(all(
    any(feature = "desktop", feature = "platform-shells"),
    not(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))
))]
pub use fission_shell_desktop::{
    BarcodeScannerHost, BiometricHost, BluetoothHost, CameraHost, ClipboardHost, DesktopApp,
    GeolocationHost, HapticHost, MemoryBarcodeScannerHost, MemoryBiometricHost,
    MemoryBluetoothHost, MemoryCameraHost, MemoryClipboardHost, MemoryGeolocationHost,
    MemoryHapticHost, MemoryMicrophoneHost, MemoryNfcHost, MemoryNotificationHost,
    MemoryPasskeyHost, MemoryVolumeHost, MemoryWifiHost, MicrophoneHost, NfcHost, NotificationHost,
    PasskeyHost, UnsupportedBarcodeScannerHost, UnsupportedBiometricHost, UnsupportedBluetoothHost,
    UnsupportedCameraHost, UnsupportedGeolocationHost, UnsupportedHapticHost,
    UnsupportedMicrophoneHost, UnsupportedNfcHost, UnsupportedNotificationHost,
    UnsupportedPasskeyHost, UnsupportedVolumeHost, UnsupportedWifiHost, VolumeHost, WifiHost,
};
#[cfg(all(
    feature = "desktop-tray",
    not(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))
))]
pub use fission_shell_desktop::{
    TrayActivateBehavior, TrayConfig, TrayHostAction, TrayIconSource, TrayMenu, TrayMenuAction,
    TrayMenuEntry, TrayMenuItem, TrayMenuWidget, WindowCloseBehavior,
};
#[cfg(all(
    any(
        feature = "android",
        feature = "ios",
        feature = "mobile",
        feature = "platform-shells"
    ),
    any(target_os = "android", target_os = "ios")
))]
pub use fission_shell_mobile::{
    BarcodeScannerHost, BiometricHost, BluetoothHost, CameraHost, ClipboardHost, GeolocationHost,
    HapticHost, MemoryBarcodeScannerHost, MemoryBiometricHost, MemoryBluetoothHost,
    MemoryCameraHost, MemoryClipboardHost, MemoryGeolocationHost, MemoryHapticHost,
    MemoryMicrophoneHost, MemoryNfcHost, MemoryNotificationHost, MemoryPasskeyHost,
    MemoryVolumeHost, MemoryWifiHost, MicrophoneHost, MobileApp, NfcHost, NotificationHost,
    PasskeyHost, UnsupportedBarcodeScannerHost, UnsupportedBiometricHost, UnsupportedBluetoothHost,
    UnsupportedCameraHost, UnsupportedGeolocationHost, UnsupportedHapticHost,
    UnsupportedMicrophoneHost, UnsupportedNfcHost, UnsupportedNotificationHost,
    UnsupportedPasskeyHost, UnsupportedVolumeHost, UnsupportedWifiHost, VolumeHost, WifiHost,
};
#[cfg(feature = "terminal-shell")]
pub use fission_shell_terminal::TerminalApp;
#[cfg(all(
    any(feature = "web", feature = "platform-shells"),
    target_arch = "wasm32"
))]
pub use fission_shell_web::{
    BarcodeScannerHost, BiometricHost, BluetoothHost, CameraHost, ClipboardHost, GeolocationHost,
    HapticHost, MemoryBarcodeScannerHost, MemoryBiometricHost, MemoryBluetoothHost,
    MemoryCameraHost, MemoryClipboardHost, MemoryGeolocationHost, MemoryHapticHost,
    MemoryMicrophoneHost, MemoryNfcHost, MemoryNotificationHost, MemoryPasskeyHost,
    MemoryVolumeHost, MemoryWifiHost, MicrophoneHost, NfcHost, NotificationHost, PasskeyHost,
    UnsupportedBarcodeScannerHost, UnsupportedBiometricHost, UnsupportedBluetoothHost,
    UnsupportedCameraHost, UnsupportedGeolocationHost, UnsupportedHapticHost,
    UnsupportedMicrophoneHost, UnsupportedNfcHost, UnsupportedNotificationHost,
    UnsupportedPasskeyHost, UnsupportedVolumeHost, UnsupportedWifiHost, VolumeHost, WebApp,
    WifiHost,
};

// Macros
pub use fission_macros::{fission_action, fission_reducer, Action as ActionDerive};

// ── Prelude ──────────────────────────────────────────────────────────────

/// Prelude for UI authoring — import this for the most common types.
pub mod prelude {
    // Widgets
    pub use fission_core::ui::*;
    pub use fission_widgets::*;

    // Actions
    pub use fission_core::env::Env;
    pub use fission_core::event::{InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
    pub use fission_core::op::{Color, Fill, PaintOp};
    pub use fission_core::{reduce, reduce_with, with_reducer};
    pub use fission_core::{
        Action, ActionEnvelope, ActionId, ActionScopeId, AnimationPropertyId, AnimationRequest,
        AnimationStartValue, AppState, AuthenticateBiometricCapability,
        BiometricAuthenticateRequest, BiometricAuthenticateResult, BiometricAvailability,
        BiometricEffects, BiometricError, BiometricKind, BiometricStrength, BuildCtx,
        CancelAllNotificationsCapability, CancelBiometricAuthenticationCapability,
        CancelNotificationCapability, CancelNotificationRequest, DeepLink, DeepLinkConfig,
        DeepLinkReceived, DeepLinkSource, Effects, EmulateNfcTagCapability, FlexDirection,
        GetBiometricAvailabilityCapability, GetNfcAvailabilityCapability,
        GetNotificationSettingsCapability, Handler, NfcAvailability, NfcEffects,
        NfcEmulationRequest, NfcError, NfcRecord, NfcRecordTypeNameFormat, NfcScanRequest,
        NfcSessionReceipt, NfcTag, NfcTagDiscovered, NfcTechnology, NfcWriteRequest, NodeBuilder,
        NotificationActionButton, NotificationEffects, NotificationError, NotificationId,
        NotificationPermission, NotificationPermissionRequest, NotificationReceipt,
        NotificationRequest, NotificationResponse, NotificationResponseReceived,
        NotificationSchedule, NotificationSettings, NotificationSound, Op, PortalLayer,
        PushPlatform, PushRegistration, PushRegistrationRequest, ReducerContext,
        RegisterPushNotificationsCapability, RequestNotificationPermissionCapability,
        ScanNfcTagCapability, ScheduleNotificationCapability, Selector, SetBadgeCountCapability,
        SetBadgeCountRequest, ShowNotificationCapability, UnregisterPushNotificationsCapability,
        View, Widget, WidgetNodeId, WindowEnv, WindowTitle, WriteNfcTagCapability,
        AUTHENTICATE_BIOMETRIC, CANCEL_ALL_NOTIFICATIONS, CANCEL_BIOMETRIC_AUTHENTICATION,
        CANCEL_NFC_SESSION, CANCEL_NOTIFICATION, EMULATE_NFC_TAG, GET_BIOMETRIC_AVAILABILITY,
        GET_NFC_AVAILABILITY, GET_NOTIFICATION_SETTINGS, REGISTER_PUSH_NOTIFICATIONS,
        REQUEST_NOTIFICATION_PERMISSION, SCAN_NFC_TAG, SCHEDULE_NOTIFICATION, SET_BADGE_COUNT,
        SHOW_NOTIFICATION, UNREGISTER_PUSH_NOTIFICATIONS, WRITE_NFC_TAG,
    };
    pub use fission_core::{
        AdjustVolumeLevelCapability, GetVolumeLevelCapability, SetVolumeLevelCapability,
        VolumeAdjustDirection, VolumeAdjustRequest, VolumeEffects, VolumeError, VolumeLevel,
        VolumeSetRequest, VolumeStream, ADJUST_VOLUME_LEVEL, GET_VOLUME_LEVEL, SET_VOLUME_LEVEL,
    };
    pub use fission_core::{
        AudioSampleFormat, CancelMicrophoneCaptureCapability, CaptureMicrophoneAudioCapability,
        GetMicrophoneAvailabilityCapability, MicrophoneAvailability, MicrophoneCapture,
        MicrophoneCaptureRequest, MicrophoneDevice, MicrophoneEffects, MicrophoneError,
        MicrophonePermission, MicrophonePermissionRequest, RequestMicrophonePermissionCapability,
        CANCEL_MICROPHONE_CAPTURE, CAPTURE_MICROPHONE_AUDIO, GET_MICROPHONE_AVAILABILITY,
        REQUEST_MICROPHONE_PERMISSION,
    };
    pub use fission_core::{
        AuthenticatePasskeyCapability, CancelPasskeyOperationCapability,
        GetPasskeyAvailabilityCapability, PasskeyAlgorithm, PasskeyAttestationConveyance,
        PasskeyAuthenticationRequest, PasskeyAuthenticationResult, PasskeyAuthenticatorAttachment,
        PasskeyAuthenticatorSelection, PasskeyAvailability, PasskeyCredentialDescriptor,
        PasskeyEffects, PasskeyError, PasskeyMediation, PasskeyRegistrationRequest,
        PasskeyRegistrationResult, PasskeyRelyingParty, PasskeyResidentKeyRequirement,
        PasskeyTransport, PasskeyUser, PasskeyUserVerification, RegisterPasskeyCapability,
        AUTHENTICATE_PASSKEY, CANCEL_PASSKEY_OPERATION, GET_PASSKEY_AVAILABILITY, REGISTER_PASSKEY,
    };
    pub use fission_core::{
        BarcodeFormat, BarcodeImageDecodeRequest, BarcodePoint, BarcodeScanRequest,
        BarcodeScanResult, BarcodeScanResults, BarcodeScannerEffects, BarcodeScannerError,
        CancelBarcodeScanCapability, DecodeBarcodeImageCapability, ScanBarcodeCapability,
        CANCEL_BARCODE_SCAN, DECODE_BARCODE_IMAGE, SCAN_BARCODE,
    };
    pub use fission_core::{
        BluetoothAdvertiseReceipt, BluetoothAdvertiseRequest, BluetoothAvailability,
        BluetoothConnectRequest, BluetoothConnection, BluetoothDevice, BluetoothDisconnectRequest,
        BluetoothEffects, BluetoothError, BluetoothMode, BluetoothPermission,
        BluetoothPermissionRequest, BluetoothReadRequest, BluetoothReadResult,
        BluetoothScanRequest, BluetoothScanResult, BluetoothStopAdvertiseRequest,
        BluetoothWriteRequest, ConnectBluetoothDeviceCapability,
        DisconnectBluetoothDeviceCapability, GetBluetoothAvailabilityCapability,
        ReadBluetoothCharacteristicCapability, RequestBluetoothPermissionCapability,
        ScanBluetoothDevicesCapability, StartBluetoothAdvertisingCapability,
        StopBluetoothAdvertisingCapability, WriteBluetoothCharacteristicCapability,
        CONNECT_BLUETOOTH_DEVICE, DISCONNECT_BLUETOOTH_DEVICE, GET_BLUETOOTH_AVAILABILITY,
        READ_BLUETOOTH_CHARACTERISTIC, REQUEST_BLUETOOTH_PERMISSION, SCAN_BLUETOOTH_DEVICES,
        START_BLUETOOTH_ADVERTISING, STOP_BLUETOOTH_ADVERTISING, WRITE_BLUETOOTH_CHARACTERISTIC,
    };
    pub use fission_core::{
        CameraAvailability, CameraCapture, CameraCaptureRequest, CameraDevice, CameraEffects,
        CameraError, CameraFacing, CameraFlashMode, CameraFlashlightRequest, CameraImageFormat,
        CameraPermission, CameraPermissionRequest, CameraResolution, CancelCameraCaptureCapability,
        CapturePhotoCapability, GetCameraAvailabilityCapability, RequestCameraPermissionCapability,
        SetCameraFlashlightCapability, CANCEL_CAMERA_CAPTURE, CAPTURE_PHOTO,
        GET_CAMERA_AVAILABILITY, REQUEST_CAMERA_PERMISSION, SET_CAMERA_FLASHLIGHT,
    };
    pub use fission_core::{
        ClearClipboardCapability, ClipboardContent, ClipboardEffects, ClipboardError,
        ClipboardItem, ClipboardText, ClipboardWriteTextRequest, ReadClipboardContentCapability,
        ReadClipboardTextCapability, WriteClipboardContentCapability, WriteClipboardTextCapability,
        CLEAR_CLIPBOARD, READ_CLIPBOARD_CONTENT, READ_CLIPBOARD_TEXT, WRITE_CLIPBOARD_CONTENT,
        WRITE_CLIPBOARD_TEXT,
    };
    pub use fission_core::{
        ConnectWifiNetworkCapability, DisconnectWifiNetworkCapability,
        GetWifiAvailabilityCapability, RequestWifiPermissionCapability, ScanWifiNetworksCapability,
        WifiAvailability, WifiConnectRequest, WifiConnection, WifiDisconnectRequest, WifiEffects,
        WifiError, WifiNetwork, WifiPermission, WifiPermissionRequest, WifiScanRequest,
        WifiScanResult, WifiSecurity, CONNECT_WIFI_NETWORK, DISCONNECT_WIFI_NETWORK,
        GET_WIFI_AVAILABILITY, REQUEST_WIFI_PERMISSION, SCAN_WIFI_NETWORKS,
    };
    pub use fission_core::{
        GeolocationEffects, GeolocationError, GeolocationPermission, GeolocationPermissionRequest,
        GeolocationPosition, GeolocationPositionRequest, GetCurrentPositionCapability,
        GetGeolocationPermissionCapability, RequestGeolocationPermissionCapability,
        GET_CURRENT_POSITION, GET_GEOLOCATION_PERMISSION, REQUEST_GEOLOCATION_PERMISSION,
    };
    pub use fission_core::{
        HapticEffects, HapticError, HapticImpactCapability, HapticImpactRequest, HapticImpactStyle,
        HapticNotificationCapability, HapticNotificationKind, HapticNotificationRequest,
        HapticPatternCapability, HapticPatternRequest, HapticPatternStep,
        HapticSelectionCapability, HAPTIC_IMPACT, HAPTIC_NOTIFICATION, HAPTIC_PATTERN,
        HAPTIC_SELECTION,
    };
    #[cfg(all(
        feature = "desktop-tray",
        not(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))
    ))]
    pub use fission_shell_desktop::{
        TrayActivateBehavior, TrayConfig, TrayHostAction, TrayIconSource, TrayMenu, TrayMenuAction,
        TrayMenuEntry, TrayMenuItem, TrayMenuWidget, WindowCloseBehavior,
    };

    // Layout
    pub use fission_layout::{LayoutPoint, LayoutRect, LayoutSize};

    // Design systems and generated themes.
    pub use fission_theme::*;

    // IR
    pub use fission_ir::op as ir_op;
    pub use fission_ir::NodeId;

    // Icons
    pub use fission_icons::material;

    // Macros
    pub use fission_macros::{fission_action, fission_reducer, Action};

    // Shell
    #[cfg(all(
        any(feature = "desktop", feature = "platform-shells"),
        not(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))
    ))]
    pub use fission_shell_desktop::{
        BarcodeScannerHost, BiometricHost, BluetoothHost, CameraHost, ClipboardHost, DesktopApp,
        GeolocationHost, HapticHost, MemoryBarcodeScannerHost, MemoryBiometricHost,
        MemoryBluetoothHost, MemoryCameraHost, MemoryClipboardHost, MemoryGeolocationHost,
        MemoryHapticHost, MemoryMicrophoneHost, MemoryNfcHost, MemoryNotificationHost,
        MemoryPasskeyHost, MemoryVolumeHost, MemoryWifiHost, MicrophoneHost, NfcHost,
        NotificationHost, PasskeyHost, UnsupportedBarcodeScannerHost, UnsupportedBiometricHost,
        UnsupportedBluetoothHost, UnsupportedCameraHost, UnsupportedGeolocationHost,
        UnsupportedHapticHost, UnsupportedMicrophoneHost, UnsupportedNfcHost,
        UnsupportedNotificationHost, UnsupportedPasskeyHost, UnsupportedVolumeHost,
        UnsupportedWifiHost, VolumeHost, WifiHost,
    };
    #[cfg(all(
        any(feature = "android", feature = "mobile", feature = "platform-shells"),
        target_os = "android"
    ))]
    pub use fission_shell_mobile::AndroidApp;
    #[cfg(all(
        any(
            feature = "android",
            feature = "ios",
            feature = "mobile",
            feature = "platform-shells"
        ),
        any(target_os = "android", target_os = "ios")
    ))]
    pub use fission_shell_mobile::{
        BarcodeScannerHost, BiometricHost, BluetoothHost, CameraHost, ClipboardHost,
        GeolocationHost, HapticHost, MemoryBarcodeScannerHost, MemoryBiometricHost,
        MemoryBluetoothHost, MemoryCameraHost, MemoryClipboardHost, MemoryGeolocationHost,
        MemoryHapticHost, MemoryMicrophoneHost, MemoryNfcHost, MemoryNotificationHost,
        MemoryPasskeyHost, MemoryVolumeHost, MemoryWifiHost, MicrophoneHost, MobileApp, NfcHost,
        NotificationHost, PasskeyHost, UnsupportedBarcodeScannerHost, UnsupportedBiometricHost,
        UnsupportedBluetoothHost, UnsupportedCameraHost, UnsupportedGeolocationHost,
        UnsupportedHapticHost, UnsupportedMicrophoneHost, UnsupportedNfcHost,
        UnsupportedNotificationHost, UnsupportedPasskeyHost, UnsupportedVolumeHost,
        UnsupportedWifiHost, VolumeHost, WifiHost,
    };
    #[cfg(feature = "site")]
    pub use fission_shell_site::*;
    #[cfg(feature = "terminal-shell")]
    pub use fission_shell_terminal::TerminalApp;
    #[cfg(all(
        any(feature = "web", feature = "platform-shells"),
        target_arch = "wasm32"
    ))]
    pub use fission_shell_web::{
        BarcodeScannerHost, BiometricHost, BluetoothHost, CameraHost, ClipboardHost,
        GeolocationHost, HapticHost, MemoryBarcodeScannerHost, MemoryBiometricHost,
        MemoryBluetoothHost, MemoryCameraHost, MemoryClipboardHost, MemoryGeolocationHost,
        MemoryHapticHost, MemoryMicrophoneHost, MemoryNfcHost, MemoryNotificationHost,
        MemoryPasskeyHost, MemoryVolumeHost, MemoryWifiHost, MicrophoneHost, NfcHost,
        NotificationHost, PasskeyHost, UnsupportedBarcodeScannerHost, UnsupportedBiometricHost,
        UnsupportedBluetoothHost, UnsupportedCameraHost, UnsupportedGeolocationHost,
        UnsupportedHapticHost, UnsupportedMicrophoneHost, UnsupportedNfcHost,
        UnsupportedNotificationHost, UnsupportedPasskeyHost, UnsupportedWifiHost, WebApp, WifiHost,
    };

    // Serde (commonly needed for actions)
    pub use serde::{Deserialize, Serialize};
}
