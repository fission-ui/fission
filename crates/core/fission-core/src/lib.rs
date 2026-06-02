//! # fission-core
//!
//! The runtime, widget system, and action/reducer architecture for the Fission UI
//! framework.
//!
//! `fission-core` provides:
//!
//! - A **declarative widget tree** built from composable primitives ([`Widget`]).
//! - A **unidirectional data-flow** pipeline: [`Action`] -> [`Runtime::dispatch`] -> reducer
//!   -> mutated [`GlobalState`].
//! - An **effect system** for async side-effects ([`Effect`], [`RuntimeEffect`]).
//! - Built-in widgets: [`Button`], [`Text`], [`TextInput`], [`Container`], [`Row`],
//!   [`Column`], [`Scroll`], [`ZStack`], [`Grid`], [`LazyColumn`], and more.
//!
//! ## Getting started
//!
//! ```rust,ignore
//! use fission_core::*;
//! use fission_core::ui::*;
//!
//! // Define application state
//! #[derive(Debug, Default)]
//! struct MyState { value: String }
//! impl GlobalState for MyState {}
//!
//! // Build a widget tree value
//! struct MyWidget;
//! impl From<MyWidget> for Widget {
//!     fn from(_: MyWidget) -> Widget {
//!         let (_, view) = fission_core::build::current::<MyState>();
//!         Text::new(&*view.state().value).into()
//!     }
//! }
//! ```

use anyhow::Result;
use lazy_static::lazy_static;
use std::any::TypeId;
use std::collections::HashMap;

extern crate self as fission_core;

pub mod action;
pub mod async_runtime;
pub mod build;
mod build_context;
pub mod capability; // New
pub mod context; // New
pub mod diff;
pub mod effect; // New
pub mod env;
pub mod event;
pub mod hit_test;
pub mod input;
pub(crate) mod lowering;
pub mod media;
pub mod platform;
pub mod platform_barcode;
pub mod platform_biometric;
pub mod platform_bluetooth;
pub mod platform_camera;
pub mod platform_clipboard;
pub mod platform_geolocation;
pub mod platform_haptics;
pub mod platform_microphone;
pub mod platform_nfc;
pub mod platform_passkey;
pub mod platform_volume;
pub mod platform_wifi;
pub mod registry;
pub mod runtime;
pub mod scrollbar;
pub mod state;
pub mod time;
pub mod ui;

pub mod view;

#[doc(hidden)]
/// Framework integration boundary for first-party shells, renderers, test
/// harnesses, and generated widget implementations.
///
/// This module is not part of the application authoring API. Application code
/// should construct `Widget` values from widget structs and components instead
/// of calling lowering helpers directly.
pub mod internal {
    pub use crate::build_context::BuildCtx;
    pub use crate::lowering::{
        build_layout_tree, wrap_zstack_child, InternalIrBuilder, InternalLoweringCx,
    };
    use crate::Widget;
    use fission_ir::WidgetId;

    pub fn custom_render_widget(node: InternalRenderNode) -> Widget {
        Widget::custom(node)
    }

    pub fn lower_widget(widget: &Widget, cx: &mut InternalLoweringCx) -> WidgetId {
        widget.lower(cx)
    }

    pub fn lower_widget_to_ir(widget: &Widget) -> fission_ir::CoreIR {
        let env = crate::Env::default();
        let runtime_state = crate::RuntimeState::default();
        let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
        widget.lower(&mut cx);
        cx.ir
    }

    pub fn widget_kind_name(widget: &Widget) -> &'static str {
        widget.kind_name()
    }

    pub fn widget_as_row(widget: &Widget) -> Option<&crate::ui::Row> {
        widget.as_row()
    }

    pub fn widget_as_column(widget: &Widget) -> Option<&crate::ui::Column> {
        widget.as_column()
    }

    pub fn widget_as_container(widget: &Widget) -> Option<&crate::ui::Container> {
        widget.as_container()
    }

    pub fn widget_as_scroll(widget: &Widget) -> Option<&crate::ui::Scroll> {
        widget.as_scroll()
    }

    pub fn widget_as_rich_text(widget: &Widget) -> Option<&crate::ui::RichText> {
        widget.as_rich_text()
    }

    pub fn widget_as_text(widget: &Widget) -> Option<&crate::ui::Text> {
        widget.as_text()
    }

    pub fn widget_as_text_input(widget: &Widget) -> Option<&crate::ui::TextInput> {
        widget.as_text_input()
    }

    pub fn widget_as_button(widget: &Widget) -> Option<&crate::ui::Button> {
        widget.as_button()
    }

    pub fn widget_as_gesture_detector(widget: &Widget) -> Option<&crate::ui::GestureDetector> {
        widget.as_gesture_detector()
    }

    pub fn widget_as_zstack(widget: &Widget) -> Option<&crate::ui::ZStack> {
        widget.as_zstack()
    }

    pub use crate::ui::custom_render::{
        downcast_render_object, CustomEventResult, CustomHitResult, CustomRender,
        CustomRenderObject,
    };
    pub use crate::ui::node::{CustomWidget, InternalRenderNode};
    pub use crate::ui::traits::{InternalLower, InternalLowerer};
}

pub mod public {
    pub mod action {
        pub use crate::action::*;
    }
    pub mod env {
        pub use crate::env::*;
    }
    pub mod event {
        pub use crate::event::*;
    }
    pub mod hit_test {
        pub use crate::hit_test::*;
    }
    pub mod registry {
        pub use crate::registry::*;
    }
    pub mod ui {
        pub use crate::ui::widgets::*;
        pub use crate::ui::Widget;

        pub mod widgets {
            pub use crate::ui::widgets::*;
        }
    }
    pub mod view {
        pub use crate::view::*;
    }

    pub use crate::action::{Action, ActionEnvelope, ActionId, ActionScopeId, GlobalState};
    pub use crate::async_runtime::{
        BoxFuture, JobCtx, JobRef, JobSpec, ResourceExecutionContext, ServiceBindings, ServiceCtx,
        ServiceRunner, ServiceSlot, ServiceSpec, ServiceType,
    };
    pub use crate::capability::{
        CapabilityCtx, CapabilityInvocationPayload, CapabilityType, OpenUrlCapability,
        OpenUrlRequest, OperationCapability, PickOpenFilesCapability, PickOpenFilesError,
        PickOpenFilesRequest, PickOpenFilesResult, PickedFile, OPEN_URL, PICK_OPEN_FILES,
    };
    pub use crate::context::{
        BarcodeScannerEffects, BiometricEffects, BluetoothEffects, CameraEffects, ClipboardEffects,
        Effects, GeolocationEffects, HapticEffects, MicrophoneEffects, NfcEffects,
        NotificationEffects, PasskeyEffects, ReducerContext, VolumeEffects, WifiEffects,
    }; // New
    pub use crate::effect::{ActionInput, Effect, EffectEnvelope, RuntimeEffect};
    pub use crate::env::{
        Clipboard, Env, ImeHandler, InteractionStateMap, RuntimeState, ScrollStateMap, WindowEnv,
        WindowTitle,
    };
    pub use crate::runtime::Runtime;
    pub use crate::state::{LocalStateKey, LocalStateStore, StateField};

    pub use crate::build::{BuildCtxHandle, ViewHandle};
    pub use crate::event::{
        InputEvent, KeyCode, KeyEvent, LifecycleEvent, PointerButton, PointerEvent,
    };
    pub use crate::platform::{
        CancelAllNotificationsCapability, CancelNotificationCapability, CancelNotificationRequest,
        DeepLink, DeepLinkConfig, DeepLinkReceived, DeepLinkSource,
        GetNotificationSettingsCapability, NotificationActionButton, NotificationError,
        NotificationId, NotificationPermission, NotificationPermissionRequest, NotificationReceipt,
        NotificationRequest, NotificationResponse, NotificationResponseReceived,
        NotificationSchedule, NotificationSettings, NotificationSound, PushPlatform,
        PushRegistration, PushRegistrationRequest, RegisterPushNotificationsCapability,
        RequestNotificationPermissionCapability, ScheduleNotificationCapability,
        SetBadgeCountCapability, SetBadgeCountRequest, ShowNotificationCapability,
        UnregisterPushNotificationsCapability, CANCEL_ALL_NOTIFICATIONS, CANCEL_NOTIFICATION,
        GET_NOTIFICATION_SETTINGS, REGISTER_PUSH_NOTIFICATIONS, REQUEST_NOTIFICATION_PERMISSION,
        SCHEDULE_NOTIFICATION, SET_BADGE_COUNT, SHOW_NOTIFICATION, UNREGISTER_PUSH_NOTIFICATIONS,
    };
    pub use crate::platform_barcode::{
        BarcodeFormat, BarcodeImageDecodeRequest, BarcodePoint, BarcodeScanRequest,
        BarcodeScanResult, BarcodeScanResults, BarcodeScannerError, CancelBarcodeScanCapability,
        DecodeBarcodeImageCapability, ScanBarcodeCapability, CANCEL_BARCODE_SCAN,
        DECODE_BARCODE_IMAGE, SCAN_BARCODE,
    };
    pub use crate::platform_biometric::{
        AuthenticateBiometricCapability, BiometricAuthenticateRequest, BiometricAuthenticateResult,
        BiometricAvailability, BiometricError, BiometricKind, BiometricStrength,
        CancelBiometricAuthenticationCapability, GetBiometricAvailabilityCapability,
        AUTHENTICATE_BIOMETRIC, CANCEL_BIOMETRIC_AUTHENTICATION, GET_BIOMETRIC_AVAILABILITY,
    };
    pub use crate::platform_bluetooth::{
        BluetoothAdvertiseReceipt, BluetoothAdvertiseRequest, BluetoothAvailability,
        BluetoothConnectRequest, BluetoothConnection, BluetoothDevice, BluetoothDisconnectRequest,
        BluetoothError, BluetoothMode, BluetoothPermission, BluetoothPermissionRequest,
        BluetoothReadRequest, BluetoothReadResult, BluetoothScanRequest, BluetoothScanResult,
        BluetoothStopAdvertiseRequest, BluetoothWriteRequest, ConnectBluetoothDeviceCapability,
        DisconnectBluetoothDeviceCapability, GetBluetoothAvailabilityCapability,
        ReadBluetoothCharacteristicCapability, RequestBluetoothPermissionCapability,
        ScanBluetoothDevicesCapability, StartBluetoothAdvertisingCapability,
        StopBluetoothAdvertisingCapability, WriteBluetoothCharacteristicCapability,
        CONNECT_BLUETOOTH_DEVICE, DISCONNECT_BLUETOOTH_DEVICE, GET_BLUETOOTH_AVAILABILITY,
        READ_BLUETOOTH_CHARACTERISTIC, REQUEST_BLUETOOTH_PERMISSION, SCAN_BLUETOOTH_DEVICES,
        START_BLUETOOTH_ADVERTISING, STOP_BLUETOOTH_ADVERTISING, WRITE_BLUETOOTH_CHARACTERISTIC,
    };
    pub use crate::platform_camera::{
        CameraAvailability, CameraCapture, CameraCaptureRequest, CameraDevice, CameraError,
        CameraFacing, CameraFlashMode, CameraFlashlightRequest, CameraImageFormat,
        CameraPermission, CameraPermissionRequest, CameraResolution, CancelCameraCaptureCapability,
        CapturePhotoCapability, GetCameraAvailabilityCapability, RequestCameraPermissionCapability,
        SetCameraFlashlightCapability, CANCEL_CAMERA_CAPTURE, CAPTURE_PHOTO,
        GET_CAMERA_AVAILABILITY, REQUEST_CAMERA_PERMISSION, SET_CAMERA_FLASHLIGHT,
    };
    pub use crate::platform_clipboard::{
        ClearClipboardCapability, ClipboardContent, ClipboardError, ClipboardItem, ClipboardText,
        ClipboardWriteTextRequest, ReadClipboardContentCapability, ReadClipboardTextCapability,
        WriteClipboardContentCapability, WriteClipboardTextCapability, CLEAR_CLIPBOARD,
        READ_CLIPBOARD_CONTENT, READ_CLIPBOARD_TEXT, WRITE_CLIPBOARD_CONTENT, WRITE_CLIPBOARD_TEXT,
    };
    pub use crate::platform_geolocation::{
        GeolocationError, GeolocationPermission, GeolocationPermissionRequest, GeolocationPosition,
        GeolocationPositionRequest, GetCurrentPositionCapability,
        GetGeolocationPermissionCapability, RequestGeolocationPermissionCapability,
        GET_CURRENT_POSITION, GET_GEOLOCATION_PERMISSION, REQUEST_GEOLOCATION_PERMISSION,
    };
    pub use crate::platform_haptics::{
        HapticError, HapticImpactCapability, HapticImpactRequest, HapticImpactStyle,
        HapticNotificationCapability, HapticNotificationKind, HapticNotificationRequest,
        HapticPatternCapability, HapticPatternRequest, HapticPatternStep,
        HapticSelectionCapability, HAPTIC_IMPACT, HAPTIC_NOTIFICATION, HAPTIC_PATTERN,
        HAPTIC_SELECTION,
    };
    pub use crate::platform_microphone::{
        AudioSampleFormat, CancelMicrophoneCaptureCapability, CaptureMicrophoneAudioCapability,
        GetMicrophoneAvailabilityCapability, MicrophoneAvailability, MicrophoneCapture,
        MicrophoneCaptureRequest, MicrophoneDevice, MicrophoneError, MicrophonePermission,
        MicrophonePermissionRequest, RequestMicrophonePermissionCapability,
        CANCEL_MICROPHONE_CAPTURE, CAPTURE_MICROPHONE_AUDIO, GET_MICROPHONE_AVAILABILITY,
        REQUEST_MICROPHONE_PERMISSION,
    };
    pub use crate::platform_nfc::{
        CancelNfcSessionCapability, EmulateNfcTagCapability, GetNfcAvailabilityCapability,
        NfcAvailability, NfcEmulationRequest, NfcError, NfcRecord, NfcRecordTypeNameFormat,
        NfcScanRequest, NfcSessionReceipt, NfcTag, NfcTagDiscovered, NfcTechnology,
        NfcWriteRequest, ScanNfcTagCapability, WriteNfcTagCapability, CANCEL_NFC_SESSION,
        EMULATE_NFC_TAG, GET_NFC_AVAILABILITY, SCAN_NFC_TAG, WRITE_NFC_TAG,
    };
    pub use crate::platform_passkey::{
        AuthenticatePasskeyCapability, CancelPasskeyOperationCapability,
        GetPasskeyAvailabilityCapability, PasskeyAlgorithm, PasskeyAttestationConveyance,
        PasskeyAuthenticationRequest, PasskeyAuthenticationResult, PasskeyAuthenticatorAttachment,
        PasskeyAuthenticatorSelection, PasskeyAvailability, PasskeyCredentialDescriptor,
        PasskeyError, PasskeyMediation, PasskeyRegistrationRequest, PasskeyRegistrationResult,
        PasskeyRelyingParty, PasskeyResidentKeyRequirement, PasskeyTransport, PasskeyUser,
        PasskeyUserVerification, RegisterPasskeyCapability, AUTHENTICATE_PASSKEY,
        CANCEL_PASSKEY_OPERATION, GET_PASSKEY_AVAILABILITY, REGISTER_PASSKEY,
    };
    pub use crate::platform_volume::{
        AdjustVolumeLevelCapability, GetVolumeLevelCapability, SetVolumeLevelCapability,
        VolumeAdjustDirection, VolumeAdjustRequest, VolumeError, VolumeLevel, VolumeSetRequest,
        VolumeStream, ADJUST_VOLUME_LEVEL, GET_VOLUME_LEVEL, SET_VOLUME_LEVEL,
    };
    pub use crate::platform_wifi::{
        ConnectWifiNetworkCapability, DisconnectWifiNetworkCapability,
        GetWifiAvailabilityCapability, RequestWifiPermissionCapability, ScanWifiNetworksCapability,
        WifiAvailability, WifiConnectRequest, WifiConnection, WifiDisconnectRequest, WifiError,
        WifiNetwork, WifiPermission, WifiPermissionRequest, WifiScanRequest, WifiScanResult,
        WifiSecurity, CONNECT_WIFI_NETWORK, DISCONNECT_WIFI_NETWORK, GET_WIFI_AVAILABILITY,
        REQUEST_WIFI_PERMISSION, SCAN_WIFI_NETWORKS,
    };
    pub use crate::registry::{
        ActionRegistry, AnimationPropertyId, AnimationRequest, AnimationStartValue, EasingFunction,
        Handler, JobResource, PortalLayer, ResourceKey, ResourcePolicy, ResourceRegistry,
        RuntimeResourceDeclaration, RuntimeResourceKind, ServiceResource, TimerResource,
        VideoRegistration,
    };
    pub use crate::time::{Clock, CurrentTime};
    pub use crate::ui::{
        provider, ActionScope, BadgeTone, Button, ButtonHierarchy, CardPattern, Column,
        ComponentSize, ComponentState, CustomWidget, Provider, Row, Text, Widget, WidgetIdExt,
    };
    pub use crate::view::{ComputedView, FissionViewField, Selector, ValueView, View};
    pub use crate::{reduce, reduce_with, widgets, with_reducer};
    pub use fission_ir::op;
    pub use fission_ir::{EmbedKind, Op, Role, Semantics, WidgetId};
    pub use fission_layout::{
        BoxConstraints, FlexDirection, LayoutEngine, LayoutOp, LayoutPoint, LayoutRect, LayoutSize,
        LayoutSnapshot, LayoutUnit, TextMeasurer,
    };
}

#[cfg(test)]
mod tests;

pub use action::{Action, ActionEnvelope, ActionId, ActionScopeId, GlobalState};
pub use async_runtime::{
    BoxFuture, JobCtx, JobRef, JobSpec, ResourceExecutionContext, ServiceBindings, ServiceCtx,
    ServiceRunner, ServiceSlot, ServiceSpec, ServiceType,
};
pub use capability::{
    CapabilityCtx, CapabilityInvocationPayload, CapabilityType, OpenUrlCapability, OpenUrlRequest,
    OperationCapability, PickOpenFilesCapability, PickOpenFilesError, PickOpenFilesRequest,
    PickOpenFilesResult, PickedFile, OPEN_URL, PICK_OPEN_FILES,
};
pub use context::{
    BarcodeScannerEffects, BiometricEffects, BluetoothEffects, CameraEffects, ClipboardEffects,
    Effects, GeolocationEffects, HapticEffects, MicrophoneEffects, NfcEffects, NotificationEffects,
    PasskeyEffects, ReducerContext, VolumeEffects, WifiEffects,
}; // New
pub use effect::{ActionInput, Effect, EffectEnvelope, RuntimeEffect};
pub use env::{
    Clipboard, Env, ImeHandler, InteractionStateMap, RuntimeState, ScrollStateMap, WindowEnv,
    WindowTitle,
};
pub use runtime::Runtime;
pub use state::{LocalStateKey, LocalStateStore, StateField};

pub use build::{BuildCtxHandle, ViewHandle};
pub use event::{InputEvent, KeyCode, KeyEvent, LifecycleEvent, PointerButton, PointerEvent};
pub use fission_ir::op;
pub use fission_ir::{EmbedKind, Op, Role, Semantics, WidgetId};
pub use fission_layout::{
    BoxConstraints, FlexDirection, LayoutEngine, LayoutOp, LayoutPoint, LayoutRect, LayoutSize,
    LayoutSnapshot, LayoutUnit, TextMeasurer,
};
pub use platform::{
    CancelAllNotificationsCapability, CancelNotificationCapability, CancelNotificationRequest,
    DeepLink, DeepLinkConfig, DeepLinkReceived, DeepLinkSource, GetNotificationSettingsCapability,
    NotificationActionButton, NotificationError, NotificationId, NotificationPermission,
    NotificationPermissionRequest, NotificationReceipt, NotificationRequest, NotificationResponse,
    NotificationResponseReceived, NotificationSchedule, NotificationSettings, NotificationSound,
    PushPlatform, PushRegistration, PushRegistrationRequest, RegisterPushNotificationsCapability,
    RequestNotificationPermissionCapability, ScheduleNotificationCapability,
    SetBadgeCountCapability, SetBadgeCountRequest, ShowNotificationCapability,
    UnregisterPushNotificationsCapability, CANCEL_ALL_NOTIFICATIONS, CANCEL_NOTIFICATION,
    GET_NOTIFICATION_SETTINGS, REGISTER_PUSH_NOTIFICATIONS, REQUEST_NOTIFICATION_PERMISSION,
    SCHEDULE_NOTIFICATION, SET_BADGE_COUNT, SHOW_NOTIFICATION, UNREGISTER_PUSH_NOTIFICATIONS,
};
pub use platform_barcode::{
    BarcodeFormat, BarcodeImageDecodeRequest, BarcodePoint, BarcodeScanRequest, BarcodeScanResult,
    BarcodeScanResults, BarcodeScannerError, CancelBarcodeScanCapability,
    DecodeBarcodeImageCapability, ScanBarcodeCapability, CANCEL_BARCODE_SCAN, DECODE_BARCODE_IMAGE,
    SCAN_BARCODE,
};
pub use platform_biometric::{
    AuthenticateBiometricCapability, BiometricAuthenticateRequest, BiometricAuthenticateResult,
    BiometricAvailability, BiometricError, BiometricKind, BiometricStrength,
    CancelBiometricAuthenticationCapability, GetBiometricAvailabilityCapability,
    AUTHENTICATE_BIOMETRIC, CANCEL_BIOMETRIC_AUTHENTICATION, GET_BIOMETRIC_AVAILABILITY,
};
pub use platform_bluetooth::{
    BluetoothAdvertiseReceipt, BluetoothAdvertiseRequest, BluetoothAvailability,
    BluetoothConnectRequest, BluetoothConnection, BluetoothDevice, BluetoothDisconnectRequest,
    BluetoothError, BluetoothMode, BluetoothPermission, BluetoothPermissionRequest,
    BluetoothReadRequest, BluetoothReadResult, BluetoothScanRequest, BluetoothScanResult,
    BluetoothStopAdvertiseRequest, BluetoothWriteRequest, ConnectBluetoothDeviceCapability,
    DisconnectBluetoothDeviceCapability, GetBluetoothAvailabilityCapability,
    ReadBluetoothCharacteristicCapability, RequestBluetoothPermissionCapability,
    ScanBluetoothDevicesCapability, StartBluetoothAdvertisingCapability,
    StopBluetoothAdvertisingCapability, WriteBluetoothCharacteristicCapability,
    CONNECT_BLUETOOTH_DEVICE, DISCONNECT_BLUETOOTH_DEVICE, GET_BLUETOOTH_AVAILABILITY,
    READ_BLUETOOTH_CHARACTERISTIC, REQUEST_BLUETOOTH_PERMISSION, SCAN_BLUETOOTH_DEVICES,
    START_BLUETOOTH_ADVERTISING, STOP_BLUETOOTH_ADVERTISING, WRITE_BLUETOOTH_CHARACTERISTIC,
};
pub use platform_camera::{
    CameraAvailability, CameraCapture, CameraCaptureRequest, CameraDevice, CameraError,
    CameraFacing, CameraFlashMode, CameraFlashlightRequest, CameraImageFormat, CameraPermission,
    CameraPermissionRequest, CameraResolution, CancelCameraCaptureCapability,
    CapturePhotoCapability, GetCameraAvailabilityCapability, RequestCameraPermissionCapability,
    SetCameraFlashlightCapability, CANCEL_CAMERA_CAPTURE, CAPTURE_PHOTO, GET_CAMERA_AVAILABILITY,
    REQUEST_CAMERA_PERMISSION, SET_CAMERA_FLASHLIGHT,
};
pub use platform_clipboard::{
    ClearClipboardCapability, ClipboardContent, ClipboardError, ClipboardItem, ClipboardText,
    ClipboardWriteTextRequest, ReadClipboardContentCapability, ReadClipboardTextCapability,
    WriteClipboardContentCapability, WriteClipboardTextCapability, CLEAR_CLIPBOARD,
    READ_CLIPBOARD_CONTENT, READ_CLIPBOARD_TEXT, WRITE_CLIPBOARD_CONTENT, WRITE_CLIPBOARD_TEXT,
};
pub use platform_geolocation::{
    GeolocationError, GeolocationPermission, GeolocationPermissionRequest, GeolocationPosition,
    GeolocationPositionRequest, GetCurrentPositionCapability, GetGeolocationPermissionCapability,
    RequestGeolocationPermissionCapability, GET_CURRENT_POSITION, GET_GEOLOCATION_PERMISSION,
    REQUEST_GEOLOCATION_PERMISSION,
};
pub use platform_haptics::{
    HapticError, HapticImpactCapability, HapticImpactRequest, HapticImpactStyle,
    HapticNotificationCapability, HapticNotificationKind, HapticNotificationRequest,
    HapticPatternCapability, HapticPatternRequest, HapticPatternStep, HapticSelectionCapability,
    HAPTIC_IMPACT, HAPTIC_NOTIFICATION, HAPTIC_PATTERN, HAPTIC_SELECTION,
};
pub use platform_microphone::{
    AudioSampleFormat, CancelMicrophoneCaptureCapability, CaptureMicrophoneAudioCapability,
    GetMicrophoneAvailabilityCapability, MicrophoneAvailability, MicrophoneCapture,
    MicrophoneCaptureRequest, MicrophoneDevice, MicrophoneError, MicrophonePermission,
    MicrophonePermissionRequest, RequestMicrophonePermissionCapability, CANCEL_MICROPHONE_CAPTURE,
    CAPTURE_MICROPHONE_AUDIO, GET_MICROPHONE_AVAILABILITY, REQUEST_MICROPHONE_PERMISSION,
};
pub use platform_nfc::{
    CancelNfcSessionCapability, EmulateNfcTagCapability, GetNfcAvailabilityCapability,
    NfcAvailability, NfcEmulationRequest, NfcError, NfcRecord, NfcRecordTypeNameFormat,
    NfcScanRequest, NfcSessionReceipt, NfcTag, NfcTagDiscovered, NfcTechnology, NfcWriteRequest,
    ScanNfcTagCapability, WriteNfcTagCapability, CANCEL_NFC_SESSION, EMULATE_NFC_TAG,
    GET_NFC_AVAILABILITY, SCAN_NFC_TAG, WRITE_NFC_TAG,
};
pub use platform_passkey::{
    AuthenticatePasskeyCapability, CancelPasskeyOperationCapability,
    GetPasskeyAvailabilityCapability, PasskeyAlgorithm, PasskeyAttestationConveyance,
    PasskeyAuthenticationRequest, PasskeyAuthenticationResult, PasskeyAuthenticatorAttachment,
    PasskeyAuthenticatorSelection, PasskeyAvailability, PasskeyCredentialDescriptor, PasskeyError,
    PasskeyMediation, PasskeyRegistrationRequest, PasskeyRegistrationResult, PasskeyRelyingParty,
    PasskeyResidentKeyRequirement, PasskeyTransport, PasskeyUser, PasskeyUserVerification,
    RegisterPasskeyCapability, AUTHENTICATE_PASSKEY, CANCEL_PASSKEY_OPERATION,
    GET_PASSKEY_AVAILABILITY, REGISTER_PASSKEY,
};
pub use platform_volume::{
    AdjustVolumeLevelCapability, GetVolumeLevelCapability, SetVolumeLevelCapability,
    VolumeAdjustDirection, VolumeAdjustRequest, VolumeError, VolumeLevel, VolumeSetRequest,
    VolumeStream, ADJUST_VOLUME_LEVEL, GET_VOLUME_LEVEL, SET_VOLUME_LEVEL,
};
pub use platform_wifi::{
    ConnectWifiNetworkCapability, DisconnectWifiNetworkCapability, GetWifiAvailabilityCapability,
    RequestWifiPermissionCapability, ScanWifiNetworksCapability, WifiAvailability,
    WifiConnectRequest, WifiConnection, WifiDisconnectRequest, WifiError, WifiNetwork,
    WifiPermission, WifiPermissionRequest, WifiScanRequest, WifiScanResult, WifiSecurity,
    CONNECT_WIFI_NETWORK, DISCONNECT_WIFI_NETWORK, GET_WIFI_AVAILABILITY, REQUEST_WIFI_PERMISSION,
    SCAN_WIFI_NETWORKS,
};
pub use registry::{
    ActionRegistry, AnimationPropertyId, AnimationRequest, AnimationStartValue, EasingFunction,
    Handler, JobResource, PortalLayer, ResourceKey, ResourcePolicy, ResourceRegistry,
    RuntimeResourceDeclaration, RuntimeResourceKind, ServiceResource, TimerResource,
    VideoRegistration,
};
pub use time::{Clock, CurrentTime};
pub use ui::{
    provider, ActionScope, BadgeTone, Button, ButtonHierarchy, CardPattern, Column, ComponentSize,
    ComponentState, CustomWidget, Provider, Row, Text, Widget, WidgetIdExt,
};
pub use view::{ComputedView, FissionViewField, Selector, ValueView, View};

/// Coerces a reducer function item or non-capturing closure to the handler
/// function-pointer type Rust can infer from the surrounding `ctx.bind(...)`
/// call.
///
/// ```rust,ignore
/// use fission::prelude::*;
///
/// let on_press = with_reducer!(ctx, Increment, on_increment);
/// ```
#[macro_export]
macro_rules! reduce_with {
    ($handler:expr $(,)?) => {
        $handler as $crate::Handler<_, _>
    };
}

/// Short alias for [`reduce_with!`].
#[macro_export]
macro_rules! reduce {
    ($handler:expr $(,)?) => {
        $crate::reduce_with!($handler)
    };
}

/// Builds a `Vec<Widget>` from widget expressions without repeated `.into()` calls.
///
/// Dynamic children may still be produced with normal iterators and
/// `collect::<Vec<Widget>>()`; this macro is only syntax sugar for
/// literal child lists.
#[macro_export]
macro_rules! widgets {
    ($($widget:expr),* $(,)?) => {
        {
            let mut widgets = ::std::vec::Vec::<$crate::Widget>::new();
            $(
                widgets.push($crate::Widget::from($widget));
            )*
            widgets
        }
    };
}

/// Binds an action to a reducer in one expression.
///
/// ```rust,ignore
/// use fission::prelude::*;
///
/// let on_press = with_reducer!(ctx, Increment, on_increment);
/// ```
#[macro_export]
macro_rules! with_reducer {
    ($ctx:expr, $action:expr, $handler:expr $(,)?) => {
        $ctx.bind($action, $crate::reduce_with!($handler))
    };
}

/// A frame-tick action that advances the runtime clock by a delta.
///
/// The platform shell dispatches `Tick` once per frame so that animations,
/// timers, and other time-dependent logic can progress.
///
/// # Example
///
/// ```rust,ignore
/// // Advance the runtime by 16 ms (~60 fps)
/// runtime.tick(16)?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tick {
    /// Delta time in milliseconds since the last tick.
    pub dt: CurrentTime,
}

impl Action for Tick {
    fn static_id() -> ActionId {
        *TICK_ACTION_ID
    }
}

lazy_static! {
    pub static ref TICK_ACTION_ID: ActionId = ActionId::from_name("fission_core::Tick");
}

/// An action that sets the runtime clock to an absolute timestamp.
///
/// Unlike [`Tick`] which advances by a delta, `AdvanceTo` jumps directly to
/// the given time. Useful for testing and deterministic replay.
///
/// # Example
///
/// ```rust,ignore
/// let envelope: ActionEnvelope = AdvanceTo { time: 5000 }.into();
/// runtime.dispatch(envelope, WidgetId::from_u128(0))?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AdvanceTo {
    /// The absolute time (in milliseconds) to set the clock to.
    pub time: CurrentTime,
}

impl Action for AdvanceTo {
    fn static_id() -> ActionId {
        *ADVANCE_TO_ACTION_ID
    }
}

lazy_static! {
    pub static ref ADVANCE_TO_ACTION_ID: ActionId = ActionId::from_name("fission_core::AdvanceTo");
}

/// A type-erased reducer function stored in the [`Runtime`].
///
/// `BoxedReducer` is the internal representation used by the runtime to invoke
/// reducers without knowing the concrete `GlobalState` or `Action` types.
pub(crate) type BoxedReducer = Box<
    dyn FnMut(
            &mut HashMap<TypeId, Box<dyn GlobalState>>,
            &ActionEnvelope,
            WidgetId,
            &mut Vec<EffectEnvelope>,
            &ActionInput,
        ) -> Result<()>
        + Send
        + Sync,
>;
