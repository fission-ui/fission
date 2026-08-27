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
//! - Built-in widgets: [`ui::Button`], [`ui::Text`], [`ui::TextInput`],
//!   [`ui::Container`], [`ui::Row`], [`ui::Column`], [`ui::Scroll`],
//!   [`ui::ZStack`], [`ui::Grid`], [`ui::LazyColumn`], and more.
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
use std::sync::{Arc, Mutex};

extern crate self as fission_core;

pub mod action;
pub mod async_runtime;
pub mod build;
mod build_context;
pub mod capability; // New
pub mod context; // New
pub mod data_stream;
pub mod diff;
pub mod effect; // New
pub mod env;
pub mod event;
pub mod hit_test;
pub mod input;
pub(crate) mod lowering;
pub mod media;
pub mod motion;
pub mod navigation;
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
pub mod scoped_action_handlers;
pub mod scrollbar;
pub mod selection;
pub mod state;
#[cfg(feature = "store")]
pub mod storage;
pub mod text_control;
pub mod text_editing;
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
        let root = cx.next_widget_root();
        lower_widget_with_root(widget, cx, root)
    }

    pub fn lower_widget_with_root(
        widget: &Widget,
        cx: &mut InternalLoweringCx,
        root: WidgetId,
    ) -> WidgetId {
        widget.clone().resolve_identities(root).lower(cx)
    }

    pub fn lower_widget_to_ir(widget: &Widget) -> fission_ir::CoreIR {
        lower_widget_to_ir_with_root(widget, WidgetId::app_root())
    }

    pub fn lower_widget_to_ir_with_root(widget: &Widget, root: WidgetId) -> fission_ir::CoreIR {
        let env = crate::Env::default();
        let runtime_state = crate::RuntimeState::default();
        let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
        let root_id = widget.clone().resolve_identities(root).lower(&mut cx);
        cx.ir.root = Some(root_id);
        cx.ir
    }

    pub fn resolve_widget_identities(widget: &Widget, root: WidgetId) -> Widget {
        widget.clone().resolve_identities(root)
    }

    pub fn shell_root_id(authored_root: WidgetId) -> WidgetId {
        WidgetId::derived(authored_root.as_u128(), &[0xF155_5E11])
    }

    pub fn widget_id(widget: &Widget) -> Option<WidgetId> {
        widget.declared_id()
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

    #[cfg(feature = "interactive-canvas")]
    pub fn widget_as_interactive_viewer(widget: &Widget) -> Option<&crate::ui::InteractiveViewer> {
        widget.as_interactive_viewer()
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
    pub mod scoped_action_handlers {
        pub use crate::scoped_action_handlers::*;
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

    pub use crate::action::{
        Action, ActionEnvelope, ActionId, ActionScopeId, GlobalState, UpdateTextInput,
        UpdateTextSelection,
    };
    pub use crate::async_runtime::{
        BoxFuture, JobCtx, JobRef, JobSpec, ResourceExecutionContext, ServiceBindings, ServiceCtx,
        ServiceRunner, ServiceSlot, ServiceSpec, ServiceType,
    };
    pub use crate::capability::{
        CapabilityCtx, CapabilityInvocationPayload, CapabilityType, OpenUrlCapability,
        OpenUrlRequest, OperationCapability, PickOpenFilesCapability, PickOpenFilesError,
        PickOpenFilesRequest, PickOpenFilesResult, PickedFile, OPEN_URL, PICK_OPEN_FILES,
    };
    #[cfg(feature = "store-sql")]
    pub use crate::context::SqlEffects;
    #[cfg(feature = "store")]
    pub use crate::context::StoreEffects;
    pub use crate::context::{
        BarcodeScannerEffects, BiometricEffects, BluetoothEffects, CameraEffects, ClipboardEffects,
        Effects, GeolocationEffects, HapticEffects, MicrophoneEffects, NfcEffects,
        NotificationEffects, PasskeyEffects, ReducerContext, VolumeEffects, WifiEffects,
    }; // New
    pub use crate::data_stream::{
        collect_data_stream, empty_data_stream, single_chunk_data_stream, BoxFissionDataStream,
        DataStreamId, DataStreamRegistry, FissionDataStream, FissionDataStreamError,
        FissionDataStreamErrorKind,
    };
    pub use crate::effect::{
        ActionInput, ActionInputCodecError, Effect, EffectEnvelope, RuntimeEffect, ScrollAlignment,
        ScrollAxis, ScrollBehavior, ScrollIntoViewRequest,
    };
    pub use crate::env::{
        Clipboard, DragSessionPayload, DragSessionState, Env, ImeHandler, InteractionStateMap,
        RuntimeState, ScrollStateMap, WindowEnv, WindowTitle,
    };
    pub use crate::runtime::Runtime;
    pub use crate::selection::{
        SelectionRegionCommand, SelectionRegionController, SelectionRegionError,
        TextRegionPosition, TextRegionSelection,
    };
    pub use crate::state::{LocalStateKey, LocalStateStore, StateField};
    #[cfg(feature = "store")]
    pub use crate::storage::*;
    pub use crate::text_editing::{
        CompleteTextInputFormatter, SharedTextInputFormatter, TextAffinity, TextEditBoundary,
        TextEditCommand, TextEditDirection, TextEditPhase, TextEditPipeline, TextEditResult,
        TextEditSource, TextEditingValue, TextPosition, TextRange, TextSelection, TextValuePhase,
    };
    pub use bytes::Bytes;
    #[cfg(feature = "store")]
    pub use fission_store::*;

    pub use crate::build::{BuildCtxHandle, ViewHandle};
    pub use crate::event::{
        ExternalDragEvent, InputEvent, KeyCode, KeyEvent, LifecycleEvent, PointerButton,
        PointerEvent, PointerId, PointerKind, PointerPhase, ScrollDeltaMode,
    };
    pub use crate::input::canvas::{
        CanvasInteraction, CanvasInteractionKind, CanvasInteractionPhase,
    };
    pub use crate::input::viewport::{
        ViewportInputKind, ViewportInteraction, ViewportInteractionPhase, ViewportStateMap,
    };
    pub use crate::motion::*;
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
        ActionRegistry, Handler, JobResource, PortalLayer, ResourceKey, ResourcePolicy,
        ResourceRegistry, RuntimeResourceDeclaration, RuntimeResourceKind, ServiceResource,
        TimerResource, VideoRegistration,
    };
    pub use crate::scoped_action_handlers::ScopedActionResolution;
    pub use crate::time::{Clock, CurrentTime};
    pub use crate::ui::{
        provider, ActionScope, BadgeTone, Button, ButtonHierarchy, ButtonMotion, CardPattern,
        Column, ComponentSize, ComponentState, CustomWidget, IosAudioSessionCategory,
        IosAudioSessionCategoryOption, IosAudioSessionMode, IosVideoAudioOptions, Pressable,
        PressableRole, PressableStyle, Provider, Responsive, ResponsiveCase, ResponsiveQuery, Row,
        Text, Video, VideoAudioActivation, VideoAudioOptions, VideoAudioPolicy, VideoSource,
        Widget, WidgetIdExt, WidgetKind,
    };
    pub use crate::view::{ComputedView, FissionViewField, Selector, ValueView, View};
    pub use crate::{
        reduce, reduce_with, video_asset, video_file, video_network, widgets, with_reducer,
    };
    pub use fission_ir::op;
    pub use fission_ir::op::{
        BoxAlignment, BoxGridPlacement, BoxPosition, BoxStyle, GridPlacement, GridTrack, Length,
        Overflow,
    };
    pub use fission_ir::{
        EmbedKind, FocusPolicy, Hyperlink, InputFormatter, LinkTarget, MaxLengthEnforcement, Op,
        PopoverAction, PopoverTarget, Role, Semantics, TextCapitalization,
        TextFieldValidationState, TextInputAction, TextInputType, TextWrapMode, WidgetId,
    };
    pub use fission_layout::{
        BoxConstraints, FlexDirection, LayoutEngine, LayoutInspection, LayoutNodeGeometry,
        LayoutOp, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, LayoutUnit, TextMeasurer,
    };
}

#[cfg(test)]
mod tests;

pub use action::{
    Action, ActionEnvelope, ActionId, ActionScopeId, GlobalState, ShellRouteChanged,
    UpdateTextInput, UpdateTextSelection,
};
pub use async_runtime::{
    BoxFuture, JobCtx, JobRef, JobSpec, ResourceExecutionContext, ServiceBindings, ServiceCtx,
    ServiceRunner, ServiceSlot, ServiceSpec, ServiceType,
};
pub use bytes::Bytes;
pub use capability::{
    CapabilityCtx, CapabilityInvocationPayload, CapabilityType, OpenUrlCapability, OpenUrlRequest,
    OperationCapability, PickOpenFilesCapability, PickOpenFilesError, PickOpenFilesRequest,
    PickOpenFilesResult, PickedFile, OPEN_URL, PICK_OPEN_FILES,
};
#[cfg(feature = "store-sql")]
pub use context::SqlEffects;
#[cfg(feature = "store")]
pub use context::StoreEffects;
pub use context::{
    BarcodeScannerEffects, BiometricEffects, BluetoothEffects, CameraEffects, ClipboardEffects,
    Effects, GeolocationEffects, HapticEffects, MicrophoneEffects, NfcEffects, NotificationEffects,
    PasskeyEffects, ReducerContext, VolumeEffects, WifiEffects,
}; // New
pub use data_stream::{
    collect_data_stream, empty_data_stream, single_chunk_data_stream, BoxFissionDataStream,
    DataStreamId, DataStreamRegistry, FissionDataStream, FissionDataStreamError,
    FissionDataStreamErrorKind,
};
pub use effect::{
    ActionInput, ActionInputCodecError, Effect, EffectEnvelope, RuntimeEffect, ScrollAlignment,
    ScrollAxis, ScrollBehavior, ScrollIntoViewRequest,
};
pub use env::{
    Clipboard, DragSessionPayload, DragSessionState, Env, ImeHandler, InteractionStateMap,
    RouteLocation, RuntimeState, ScrollStateMap, WindowEnv, WindowTitle,
};
#[cfg(feature = "store")]
pub use fission_store::*;
pub use motion::*;
pub use navigation::{
    NavigationCommand, NavigationRequested, RouteBuildOutcome, RouteDecision, RouteRedirect,
    RouteRedirectHistory,
};
pub use runtime::Runtime;
pub use selection::{
    SelectionRegionCommand, SelectionRegionController, SelectionRegionError, TextRegionPosition,
    TextRegionSelection,
};
pub use state::{LocalStateKey, LocalStateStore, StateField};
#[cfg(feature = "store")]
pub use storage::*;
pub use text_control::{
    TextControlError, TextEditingCommand, TextEditingController, TextFormController,
    TextFormValidation, TextScrollCommand, TextScrollController,
};
pub use text_editing::{
    CompleteTextInputFormatter, SharedTextInputFormatter, TextAffinity, TextEditBoundary,
    TextEditCommand, TextEditDirection, TextEditPhase, TextEditPipeline, TextEditResult,
    TextEditSource, TextEditingValue, TextPosition, TextRange, TextSelection, TextValuePhase,
};

pub use build::{BuildCtxHandle, ViewHandle};
pub use event::{
    ExternalDragEvent, InputEvent, KeyCode, KeyEvent, LifecycleEvent, PointerButton, PointerEvent,
    PointerId, PointerKind, PointerPhase, ScrollDeltaMode,
};
pub use fission_ir::op;
pub use fission_ir::op::{
    BoxAlignment, BoxGridPlacement, BoxPosition, BoxStyle, GridPlacement, GridTrack, Length,
    Overflow,
};
pub use fission_ir::{
    EmbedKind, FocusPolicy, Hyperlink, InputFormatter, LinkTarget, MaxLengthEnforcement, Op,
    PopoverAction, PopoverTarget, Role, Semantics, TextCapitalization, TextFieldValidationState,
    TextInputAction, TextInputType, TextWrapMode, WidgetId,
};
pub use fission_layout::{
    BoxConstraints, FlexDirection, LayoutEngine, LayoutInspection, LayoutNodeGeometry, LayoutOp,
    LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, LayoutUnit, TextMeasurer,
};
pub use input::canvas::{CanvasInteraction, CanvasInteractionKind, CanvasInteractionPhase};
pub use input::viewport::{
    ViewportInputKind, ViewportInteraction, ViewportInteractionPhase, ViewportStateMap,
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
    ActionRegistry, Handler, JobResource, PortalLayer, ResourceKey, ResourcePolicy,
    ResourceRegistry, RuntimeResourceDeclaration, RuntimeResourceKind, ServiceResource,
    TimerResource, VideoRegistration,
};
pub use scoped_action_handlers::ScopedActionResolution;
pub use time::{Clock, CurrentTime};
pub use ui::{
    provider, ActionScope, BadgeTone, Button, ButtonHierarchy, ButtonMotion, CardPattern, Column,
    ComponentSize, ComponentState, CustomWidget, IosAudioSessionCategory,
    IosAudioSessionCategoryOption, IosAudioSessionMode, IosVideoAudioOptions, Pressable,
    PressableRole, PressableStyle, Provider, Responsive, ResponsiveCase, ResponsiveQuery, Row,
    Text, Video, VideoAudioActivation, VideoAudioOptions, VideoAudioPolicy, VideoSource, Widget,
    WidgetIdExt, WidgetKind,
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
/// `collect::<Vec<Widget>>()`. In both forms, children without explicit IDs
/// receive deterministic position-based identity. Set an explicit ID on a
/// logical collection item when retained state should follow the item through
/// reordering.
#[macro_export]
macro_rules! widgets {
    ($($widget:expr),* $(,)?) => {
        {
            let mut widgets = ::std::vec::Vec::<$crate::Widget>::new();
            let collection_scope = $crate::build::collection_scope(file!(), line!(), column!());
            $(
                let child_index = widgets.len() as u32;
                let child_scope = $crate::WidgetId::derived(
                    collection_scope.as_u128(),
                    &[child_index],
                );
                widgets.push($crate::build::with_implicit_widget_id(child_scope, || {
                    $crate::Widget::from($widget)
                }));
            )*
            widgets
        }
    };
}

/// Creates a [`Video`](crate::ui::Video) from an app asset literal and fails
/// compilation when the asset does not exist under `CARGO_MANIFEST_DIR`.
///
/// Use [`Video::asset`](crate::ui::Video::asset) when the path is computed at
/// runtime and cannot be checked by the compiler.
#[macro_export]
macro_rules! video_asset {
    ($path:literal $(,)?) => {{
        const _: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path));
        $crate::ui::Video::asset($path)
    }};
}

/// Creates a [`Video`](crate::ui::Video) from a compile-time local file path
/// and fails compilation when that path cannot be resolved by `include_bytes!`.
///
/// Relative paths are resolved the same way as `include_bytes!`: relative to the
/// source file that invokes this macro.
#[macro_export]
macro_rules! video_file {
    ($path:expr $(,)?) => {{
        const _: &[u8] = include_bytes!($path);
        $crate::ui::Video::file($path)
    }};
}

/// Creates a [`Video`](crate::ui::Video) from a network URL literal.
///
/// Network playback support is shell-specific; use this helper for literal URLs
/// and [`Video::network`](crate::ui::Video::network) when the URL is computed at
/// runtime.
#[macro_export]
macro_rules! video_network {
    ($url:literal $(,)?) => {{
        $crate::ui::Video::network($url)
    }};
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
            &Arc<EffectCallbackRegistry>,
        ) -> Result<()>
        + Send
        + Sync,
>;

/// One-shot reducers bound to async effect completion actions.
///
/// These reducers are separate from the per-frame widget registry so a
/// completion remains deliverable after the frame that issued the effect.
pub(crate) struct EffectCallbackRegistry {
    reducers: Mutex<HashMap<ActionId, Vec<BoxedReducer>>>,
}

impl EffectCallbackRegistry {
    pub(crate) fn new() -> Self {
        Self {
            reducers: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn register(&self, action_id: ActionId, reducer: BoxedReducer) {
        self.reducers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(action_id)
            .or_default()
            .push(reducer);
    }

    pub(crate) fn take(&self, action_id: ActionId) -> Vec<BoxedReducer> {
        self.reducers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&action_id)
            .unwrap_or_default()
    }

    pub(crate) fn clear(&self) -> usize {
        let mut reducers = self
            .reducers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let callback_count = reducers.values().map(Vec::len).sum();
        reducers.clear();
        callback_count
    }
}
