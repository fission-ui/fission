use fission_core::{
    BarcodeFormat, BarcodeImageDecodeRequest, BarcodePoint, BarcodeScanRequest, BarcodeScanResult,
    BarcodeScanResults, BarcodeScannerError, BluetoothAdvertiseReceipt, BluetoothAdvertiseRequest,
    BluetoothAvailability, BluetoothConnectRequest, BluetoothConnection, BluetoothDevice,
    BluetoothDisconnectRequest, BluetoothError, BluetoothMode, BluetoothPermission,
    BluetoothPermissionRequest, BluetoothReadRequest, BluetoothReadResult, BluetoothScanRequest,
    BluetoothScanResult, BluetoothStopAdvertiseRequest, BluetoothWriteRequest, CameraAvailability,
    CameraCapture, CameraCaptureRequest, CameraDevice, CameraError, CameraFacing,
    CameraFlashlightRequest, CameraImageFormat, CameraPermission, CameraPermissionRequest,
    ClipboardContent, ClipboardError, ClipboardItem, ClipboardText, ClipboardWriteTextRequest,
    GeolocationError, GeolocationPermission, GeolocationPermissionRequest, GeolocationPosition,
    GeolocationPositionRequest, HapticError, HapticImpactRequest, HapticImpactStyle,
    HapticNotificationRequest, HapticPatternRequest, HapticPatternStep, MicrophoneAvailability,
    MicrophoneCapture, MicrophoneCaptureRequest, MicrophoneDevice, MicrophoneError,
    MicrophonePermission, MicrophonePermissionRequest, NfcAvailability, NfcEmulationRequest,
    NfcError, NfcRecord, NfcRecordTypeNameFormat, NfcScanRequest, NfcSessionReceipt, NfcTag,
    NfcTechnology, NfcWriteRequest, NotificationError, NotificationPermission,
    NotificationPermissionRequest, NotificationReceipt, NotificationRequest, NotificationSchedule,
    NotificationSettings, PasskeyAuthenticationRequest, PasskeyAuthenticationResult,
    PasskeyAuthenticatorAttachment, PasskeyAvailability, PasskeyCredentialDescriptor, PasskeyError,
    PasskeyMediation, PasskeyRegistrationRequest, PasskeyRegistrationResult, PasskeyTransport,
    PasskeyUserVerification, PushPlatform, PushRegistration, PushRegistrationRequest,
    SetBadgeCountRequest, VolumeError, VolumeLevel, VolumeStream, WifiAvailability, WifiError,
    WifiPermission, ADJUST_VOLUME_LEVEL, AUTHENTICATE_BIOMETRIC, AUTHENTICATE_PASSKEY,
    CANCEL_ALL_NOTIFICATIONS, CANCEL_BARCODE_SCAN, CANCEL_BIOMETRIC_AUTHENTICATION,
    CANCEL_CAMERA_CAPTURE, CANCEL_MICROPHONE_CAPTURE, CANCEL_NFC_SESSION, CANCEL_NOTIFICATION,
    CANCEL_PASSKEY_OPERATION, CAPTURE_MICROPHONE_AUDIO, CAPTURE_PHOTO, CLEAR_CLIPBOARD,
    CONNECT_BLUETOOTH_DEVICE, CONNECT_WIFI_NETWORK, DECODE_BARCODE_IMAGE,
    DISCONNECT_BLUETOOTH_DEVICE, DISCONNECT_WIFI_NETWORK, EMULATE_NFC_TAG,
    GET_BIOMETRIC_AVAILABILITY, GET_BLUETOOTH_AVAILABILITY, GET_CAMERA_AVAILABILITY,
    GET_CURRENT_POSITION, GET_GEOLOCATION_PERMISSION, GET_MICROPHONE_AVAILABILITY,
    GET_NFC_AVAILABILITY, GET_NOTIFICATION_SETTINGS, GET_PASSKEY_AVAILABILITY, GET_VOLUME_LEVEL,
    GET_WIFI_AVAILABILITY, HAPTIC_IMPACT, HAPTIC_NOTIFICATION, HAPTIC_PATTERN, HAPTIC_SELECTION,
    READ_BLUETOOTH_CHARACTERISTIC, READ_CLIPBOARD_CONTENT, READ_CLIPBOARD_TEXT, REGISTER_PASSKEY,
    REGISTER_PUSH_NOTIFICATIONS, REQUEST_BLUETOOTH_PERMISSION, REQUEST_CAMERA_PERMISSION,
    REQUEST_GEOLOCATION_PERMISSION, REQUEST_MICROPHONE_PERMISSION, REQUEST_NOTIFICATION_PERMISSION,
    REQUEST_WIFI_PERMISSION, SCAN_BARCODE, SCAN_BLUETOOTH_DEVICES, SCAN_NFC_TAG,
    SCAN_WIFI_NETWORKS, SCHEDULE_NOTIFICATION, SET_BADGE_COUNT, SET_CAMERA_FLASHLIGHT,
    SET_VOLUME_LEVEL, SHOW_NOTIFICATION, START_BLUETOOTH_ADVERTISING, STOP_BLUETOOTH_ADVERTISING,
    UNREGISTER_PUSH_NOTIFICATIONS, WRITE_BLUETOOTH_CHARACTERISTIC, WRITE_CLIPBOARD_CONTENT,
    WRITE_CLIPBOARD_TEXT, WRITE_NFC_TAG,
};
use fission_core::{BiometricAvailability, BiometricError};
use fission_shell::async_host::AsyncRegistry;
use js_sys::{Array, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(inline_js = r#"
const fissionBluetoothDevices = new Map();
const fissionBluetoothConnections = new Map();
let fissionNfcAbortController = null;
let fissionTorchStream = null;
const fissionNotificationTimers = new Map();

function unsupported(message) {
  const error = new Error(message);
  error.name = "unsupported";
  return error;
}

function timeout(message) {
  const error = new Error(message);
  error.name = "timeout";
  return error;
}

function stopStream(stream) {
  if (stream && stream.getTracks) {
    for (const track of stream.getTracks()) track.stop();
  }
}

function dataUrlToBytes(dataUrl) {
  const comma = dataUrl.indexOf(",");
  const meta = dataUrl.slice(0, comma);
  const binary = atob(dataUrl.slice(comma + 1));
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  const contentType = (meta.match(/^data:([^;]+)/) || [])[1] || "image/png";
  return { bytes, contentType };
}

function normalizeBarcodeFormat(format) {
  switch (format) {
    case "QrCode": return "qr_code";
    case "DataMatrix": return "data_matrix";
    case "Ean13": return "ean_13";
    case "Ean8": return "ean_8";
    case "Code128": return "code_128";
    case "Code39": return "code_39";
    case "Code93": return "code_93";
    case "UpcA": return "upc_a";
    case "UpcE": return "upc_e";
    case "Aztec": return "aztec";
    case "Codabar": return "codabar";
    case "Itf": return "itf";
    case "Pdf417": return "pdf417";
    default: return String(format || "").toInternalLowerCase();
  }
}

function denormalizeBarcodeFormat(format) {
  switch (format) {
    case "qr_code": return "QrCode";
    case "data_matrix": return "DataMatrix";
    case "ean_13": return "Ean13";
    case "ean_8": return "Ean8";
    case "code_128": return "Code128";
    case "code_39": return "Code39";
    case "code_93": return "Code93";
    case "upc_a": return "UpcA";
    case "upc_e": return "UpcE";
    case "aztec": return "Aztec";
    case "codabar": return "Codabar";
    case "itf": return "Itf";
    case "pdf417": return "Pdf417";
    default: return String(format || "unknown");
  }
}

function barcodeDetector(formats) {
  if (!("BarcodeDetector" in globalThis)) throw unsupported("BarcodeDetector is not available in this browser");
  const normalized = (formats || []).map(normalizeBarcodeFormat).filter(Boolean);
  try {
    return normalized.length > 0 ? new BarcodeDetector({ formats: normalized }) : new BarcodeDetector();
  } catch (_) {
    return new BarcodeDetector();
  }
}

function mapBarcodeResults(results) {
  const encoder = new TextEncoder();
  return Array.from(results || []).map((item) => ({
    value: item.rawValue || "",
    format: denormalizeBarcodeFormat(item.format),
    rawBytes: encoder.encode(item.rawValue || ""),
    bounds: Array.from(item.cornerPoints || []).map((point) => ({
      x: Math.round(point.x || 0),
      y: Math.round(point.y || 0),
    })),
    symbologyIdentifier: null,
  }));
}

async function mediaDevices() {
  if (!navigator.mediaDevices) throw unsupported("mediaDevices is not available in this browser");
  return navigator.mediaDevices;
}

async function enumerateMedia(kind) {
  const devices = await (await mediaDevices()).enumerateDevices();
  return devices
    .filter((device) => device.kind === kind)
    .map((device, index) => ({
      id: device.deviceId || `${kind}-${index}`,
      label: device.label || null,
      kind: device.kind,
      facing: device.label && /front|user/i.test(device.label) ? "front" : (device.label && /back|rear|environment/i.test(device.label) ? "back" : "unspecified"),
    }));
}

function videoConstraints(facing, width, height) {
  const video = {};
  if (facing === "front") video.facingMode = { ideal: "user" };
  if (facing === "back") video.facingMode = { ideal: "environment" };
  if (width > 0) video.width = { ideal: width };
  if (height > 0) video.height = { ideal: height };
  return { video: Object.keys(video).length ? video : true };
}

async function captureVideoFrame(facing, width, height, mimeType, quality) {
  const devices = await mediaDevices();
  const stream = await devices.getUserMedia(videoConstraints(facing, width, height));
  try {
    const video = document.createElement("video");
    video.muted = true;
    video.playsInline = true;
    video.srcObject = stream;
    await new Promise((resolve, reject) => {
      video.onloadedmetadata = resolve;
      video.onerror = () => reject(new Error("video metadata failed to load"));
      video.play().catch(reject);
    });
    const canvas = document.createElement("canvas");
    canvas.width = width > 0 ? width : (video.videoWidth || 640);
    canvas.height = height > 0 ? height : (video.videoHeight || 480);
    const context = canvas.getContext("2d");
    context.drawImage(video, 0, 0, canvas.width, canvas.height);
    const dataUrl = canvas.toDataURL(mimeType || "image/png", quality > 0 ? quality / 100 : undefined);
    const payload = dataUrlToBytes(dataUrl);
    return {
      bytes: payload.bytes,
      contentType: payload.contentType,
      width: canvas.width,
      height: canvas.height,
      deviceId: (stream.getVideoTracks()[0] && stream.getVideoTracks()[0].getSettings().deviceId) || null,
    };
  } finally {
    stopStream(stream);
  }
}

function passkeyBytes(value) {
  return new Uint8Array(value || []);
}

function coseAlgorithm(name) {
  switch (name) {
    case "ES256": return -7;
    case "RS256": return -257;
    case "EdDSA": return -8;
    default:
      if (name && typeof name === "object" && typeof name.Other === "number") return name.Other;
      return -7;
  }
}

function pubKeyCredentialParameters(algorithms) {
  const ids = [];
  const pushUnique = (id) => {
    if (!ids.includes(id)) ids.push(id);
  };
  for (const algorithm of Array.isArray(algorithms) ? algorithms : []) {
    pushUnique(coseAlgorithm(algorithm));
  }
  pushUnique(-7);
  pushUnique(-257);
  return ids.map((alg) => ({ type: "public-key", alg }));
}

function webauthnRelyingParty(request) {
  const rp = { name: request.relying_party.name };
  if (request.relying_party.id) rp.id = request.relying_party.id;
  return rp;
}

function webauthnRpId(request) {
  return request.relying_party_id || undefined;
}

function userVerification(value) {
  switch (value) {
    case "Required": return "required";
    case "Discouraged": return "discouraged";
    case "Preferred": return "preferred";
    default: return "preferred";
  }
}

function mediation(value) {
  switch (value) {
    case "Silent": return "silent";
    case "Optional": return "optional";
    case "Conditional": return "conditional";
    case "Required": return "required";
    default: return undefined;
  }
}

function attestation(value) {
  switch (value) {
    case "Indirect": return "indirect";
    case "Direct": return "direct";
    case "Enterprise": return "enterprise";
    default: return "none";
  }
}

function attachment(value) {
  switch (value) {
    case "Platform": return "platform";
    case "CrossPlatform": return "cross-platform";
    default: return undefined;
  }
}

function transport(value) {
  switch (value) {
    case "Usb": return "usb";
    case "Nfc": return "nfc";
    case "Ble": return "ble";
    case "Internal": return "internal";
    case "Hybrid": return "hybrid";
    default: return undefined;
  }
}

function credentialDescriptor(item) {
  return {
    type: "public-key",
    id: passkeyBytes(item.id),
    transports: (item.transports || []).map(transport).filter(Boolean),
  };
}

async function publicKeyCredential() {
  if (!navigator.credentials || !("PublicKeyCredential" in globalThis)) {
    throw unsupported("WebAuthn is not available in this browser");
  }
  return globalThis.PublicKeyCredential;
}

export function fissionNotificationPermission() {
  return "Notification" in globalThis ? Notification.permission : "unsupported";
}

export function fissionRequestNotificationPermission() {
  if (!("Notification" in globalThis)) return Promise.resolve("unsupported");
  return Notification.requestPermission();
}

async function ensureNotificationPermission() {
  if (!("Notification" in globalThis)) throw unsupported("Notification is not available in this browser");
  if (Notification.permission === "granted") return;
  if (Notification.permission === "default") await Notification.requestPermission();
  if (Notification.permission !== "granted") {
    throw Object.assign(new Error("notification permission is not granted"), { name: "permission_denied" });
  }
}

export function fissionShowNotification(id, title, body, silent) {
  return (async () => {
    await ensureNotificationPermission();
    const notification = new Notification(title, { body, tag: id, silent });
    return { id, delivered: true };
  })();
}

export function fissionScheduleNotification(id, title, body, silent, delayMs) {
  return (async () => {
    await ensureNotificationPermission();
    if (fissionNotificationTimers.has(id)) {
      clearTimeout(fissionNotificationTimers.get(id));
      fissionNotificationTimers.delete(id);
    }
    const delay = Math.max(0, Number(delayMs || 0));
    const timer = setTimeout(() => {
      try {
        new Notification(title, { body, tag: id, silent });
      } finally {
        fissionNotificationTimers.delete(id);
      }
    }, delay);
    fissionNotificationTimers.set(id, timer);
    return { id, scheduled: delay > 0, delivered: delay === 0 };
  })();
}

export function fissionCancelNotification(id) {
  if (fissionNotificationTimers.has(id)) {
    clearTimeout(fissionNotificationTimers.get(id));
    fissionNotificationTimers.delete(id);
  }
  return Promise.resolve();
}

export function fissionCancelAllNotifications() {
  for (const timer of fissionNotificationTimers.values()) clearTimeout(timer);
  fissionNotificationTimers.clear();
  return Promise.resolve();
}

export function fissionSetAppBadge(count) {
  if (count == null && navigator.clearAppBadge) return navigator.clearAppBadge();
  if (navigator.setAppBadge) return navigator.setAppBadge(count || 0);
  return Promise.reject(unsupported("app badge is not available in this browser"));
}

export function fissionClipboardReadText() {
  if (!navigator.clipboard || !navigator.clipboard.readText) return Promise.reject(unsupported("clipboard readText is not available"));
  return navigator.clipboard.readText();
}

export function fissionClipboardWriteText(text) {
  if (!navigator.clipboard || !navigator.clipboard.writeText) return Promise.reject(unsupported("clipboard writeText is not available"));
  return navigator.clipboard.writeText(text);
}

export function fissionGeolocationPermission() {
  if (!navigator.geolocation) return Promise.resolve("unsupported");
  if (!navigator.permissions || !navigator.permissions.query) return Promise.resolve("unknown");
  return navigator.permissions.query({ name: "geolocation" }).then((result) => result.state).catch(() => "unknown");
}

export function fissionCurrentPosition(highAccuracy, timeoutMs, maximumAgeMs) {
  if (!navigator.geolocation) return Promise.reject(unsupported("geolocation is not available"));
  return new Promise((resolve, reject) => {
    navigator.geolocation.getCurrentPosition(
      (position) => resolve({
        latitude: position.coords.latitude,
        longitude: position.coords.longitude,
        altitude: position.coords.altitude,
        accuracy: position.coords.accuracy,
        altitudeAccuracy: position.coords.altitudeAccuracy,
        heading: position.coords.heading,
        speed: position.coords.speed,
        timestamp: position.timestamp,
      }),
      (error) => reject(Object.assign(new Error(error.message), { name: error.code === 1 ? "permission_denied" : (error.code === 3 ? "timeout" : "position_unavailable") })),
      {
        enableHighAccuracy: highAccuracy,
        timeout: timeoutMs > 0 ? timeoutMs : Infinity,
        maximumAge: maximumAgeMs >= 0 ? maximumAgeMs : 0,
      },
    );
  });
}

export function fissionVibrate(pattern) {
  if (!navigator.vibrate) return Promise.reject(unsupported("vibration haptics are not available"));
  const ok = navigator.vibrate(pattern);
  return ok ? Promise.resolve(true) : Promise.reject(unsupported("vibration haptics were rejected"));
}

export function fissionMediaAvailability(kind) {
  return enumerateMedia(kind).then((devices) => ({ permission: devices.some((device) => !!device.label) ? "granted" : "unknown", devices }));
}

export function fissionRequestMedia(kind) {
  return mediaDevices().then((devices) => devices.getUserMedia(kind === "audioinput" ? { audio: true } : { video: true }))
    .then((stream) => { stopStream(stream); return "granted"; });
}

export function fissionCapturePhoto(facing, width, height, mimeType, quality) {
  return captureVideoFrame(facing, width, height, mimeType, quality);
}

export function fissionSetCameraTorch(enabled) {
  return (async () => {
    if (!enabled) {
      stopStream(fissionTorchStream);
      fissionTorchStream = null;
      return true;
    }
    stopStream(fissionTorchStream);
    fissionTorchStream = await (await mediaDevices()).getUserMedia({ video: { facingMode: { ideal: "environment" } } });
    const track = fissionTorchStream.getVideoTracks()[0];
    if (!track) {
      stopStream(fissionTorchStream);
      fissionTorchStream = null;
      throw unsupported("camera torch track is not available");
    }
    const capabilities = track.getCapabilities ? track.getCapabilities() : {};
    if (!capabilities || !capabilities.torch || !track.applyConstraints) {
      stopStream(fissionTorchStream);
      fissionTorchStream = null;
      throw unsupported("camera torch is not available in this browser/device");
    }
    await track.applyConstraints({ advanced: [{ torch: true }] });
    return true;
  })();
}

export function fissionCaptureAudio(durationMs) {
  if (!("MediaRecorder" in globalThis)) return Promise.reject(unsupported("MediaRecorder is not available in this browser"));
  return mediaDevices().then((devices) => devices.getUserMedia({ audio: true })).then((stream) => new Promise((resolve, reject) => {
    const chunks = [];
    let recorder;
    try {
      recorder = new MediaRecorder(stream);
    } catch (error) {
      stopStream(stream);
      reject(error);
      return;
    }
    recorder.ondataavailable = (event) => { if (event.data && event.data.size > 0) chunks.push(event.data); };
    recorder.onerror = (event) => { stopStream(stream); reject(event.error || new Error("media recorder failed")); };
    recorder.onstop = async () => {
      try {
        const blob = new Blob(chunks, { type: recorder.mimeType || "audio/webm" });
        const bytes = new Uint8Array(await blob.arrayBuffer());
        stopStream(stream);
        resolve({ bytes, contentType: blob.type || "audio/webm", durationMs: durationMs || 1000 });
      } catch (error) {
        stopStream(stream);
        reject(error);
      }
    };
    recorder.start();
    setTimeout(() => recorder.state !== "inactive" && recorder.stop(), Math.max(1, durationMs || 1000));
  }));
}

export function fissionBarcodeAvailable() {
  return "BarcodeDetector" in globalThis;
}

export function fissionBarcodeDecode(bytes, contentType, formats) {
  return (async () => {
    const detector = barcodeDetector(formats);
    const bitmap = await createImageBitmap(new Blob([bytes], { type: contentType || "image/png" }));
    const results = await detector.detect(bitmap);
    if (bitmap.close) bitmap.close();
    return mapBarcodeResults(results);
  })();
}

export function fissionBarcodeScan(formats, timeoutMs, allowMultiple) {
  return (async () => {
    const detector = barcodeDetector(formats);
    const stream = await (await mediaDevices()).getUserMedia({ video: { facingMode: { ideal: "environment" } } });
    const video = document.createElement("video");
    video.muted = true;
    video.playsInline = true;
    video.srcObject = stream;
    await new Promise((resolve, reject) => {
      video.onloadedmetadata = resolve;
      video.onerror = () => reject(new Error("video metadata failed to load"));
      video.play().catch(reject);
    });
    const deadline = Date.now() + (timeoutMs > 0 ? timeoutMs : 15000);
    try {
      while (Date.now() < deadline) {
        const results = await detector.detect(video);
        if (results && results.length) return mapBarcodeResults(allowMultiple ? results : results.slice(0, 1));
        await new Promise((resolve) => requestAnimationFrame(resolve));
      }
      throw timeout("barcode scan timed out");
    } finally {
      stopStream(stream);
    }
  })();
}

export function fissionNfcAvailability() {
  const supported = "NDEFReader" in globalThis;
  return { supported, enabled: supported, read: supported, write: supported, cardEmulation: false };
}

export function fissionNfcScan(timeoutMs) {
  if (!("NDEFReader" in globalThis)) return Promise.reject(unsupported("Web NFC is not available in this browser"));
  return new Promise(async (resolve, reject) => {
    const reader = new NDEFReader();
    fissionNfcAbortController = new AbortController();
    const timer = setTimeout(() => {
      fissionNfcAbortController.abort();
      reject(timeout("NFC scan timed out"));
    }, timeoutMs > 0 ? timeoutMs : 30000);
    reader.onreading = (event) => {
      clearTimeout(timer);
      const records = Array.from(event.message.records || []).map((record) => ({
        recordType: record.recordType || "",
        mediaType: record.mediaType || "",
        id: new TextEncoder().encode(record.id || ""),
        data: record.data ? new Uint8Array(record.data.buffer) : new Uint8Array(),
      }));
      resolve({ serialNumber: event.serialNumber || null, records });
    };
    reader.onreadingerror = () => {
      clearTimeout(timer);
      reject(new Error("failed to read NFC tag"));
    };
    try {
      await reader.scan({ signal: fissionNfcAbortController.signal });
    } catch (error) {
      clearTimeout(timer);
      reject(error);
    }
  });
}

export function fissionNfcWrite(recordsJson) {
  if (!("NDEFReader" in globalThis)) return Promise.reject(unsupported("Web NFC is not available in this browser"));
  return (async () => {
    const reader = new NDEFReader();
    const records = JSON.parse(recordsJson).map((record) => ({ recordType: "mime", mediaType: "application/octet-stream", data: new Uint8Array(record.payload || []) }));
    await reader.write({ records });
    return { sessionId: null, completed: true };
  })();
}

export function fissionNfcCancel() {
  if (fissionNfcAbortController) fissionNfcAbortController.abort();
  fissionNfcAbortController = null;
  return Promise.resolve(true);
}

export function fissionBluetoothAvailability() {
  if (!navigator.bluetooth) return Promise.resolve({ supported: false, enabled: false });
  if (navigator.bluetooth.getAvailability) {
    return navigator.bluetooth.getAvailability().then((enabled) => ({ supported: true, enabled }));
  }
  return Promise.resolve({ supported: true, enabled: true });
}

export function fissionBluetoothScan(services) {
  if (!navigator.bluetooth || !navigator.bluetooth.requestDevice) return Promise.reject(unsupported("Web Bluetooth is not available"));
  const serviceList = Array.from(services || []);
  const options = serviceList.length
    ? { filters: serviceList.map((service) => ({ services: [service] })), optionalServices: serviceList }
    : { acceptAllDevices: true, optionalServices: [] };
  return navigator.bluetooth.requestDevice(options).then((device) => {
    fissionBluetoothDevices.set(device.id, device);
    return [{ id: device.id, name: device.name || null, address: null, rssi: null, paired: false, modes: ["LowEnergy"] }];
  });
}

export function fissionBluetoothConnect(deviceId, services) {
  const device = fissionBluetoothDevices.get(deviceId);
  if (!device) return Promise.reject(new Error(`Bluetooth device ${deviceId} is not known; scan first`));
  if (!device.gatt) return Promise.reject(unsupported("Bluetooth GATT is not available for this device"));
  return device.gatt.connect().then((server) => {
    const connectionId = `web:${device.id}`;
    fissionBluetoothConnections.set(connectionId, { device, server });
    return { connectionId, device: { id: device.id, name: device.name || null, address: null, rssi: null, paired: false, modes: ["LowEnergy"] } };
  });
}

export function fissionBluetoothDisconnect(connectionId) {
  const entry = fissionBluetoothConnections.get(connectionId);
  if (entry && entry.device && entry.device.gatt && entry.device.gatt.connected) entry.device.gatt.disconnect();
  fissionBluetoothConnections.delete(connectionId);
  return Promise.resolve(true);
}

export function fissionBluetoothRead(connectionId, serviceUuid, characteristicUuid) {
  const entry = fissionBluetoothConnections.get(connectionId);
  if (!entry) return Promise.reject(new Error(`Bluetooth connection ${connectionId} is not known`));
  return entry.server.getPrimaryService(serviceUuid)
    .then((service) => service.getCharacteristic(characteristicUuid))
    .then((characteristic) => characteristic.readValue())
    .then((value) => new Uint8Array(value.buffer));
}

export function fissionBluetoothWrite(connectionId, serviceUuid, characteristicUuid, value, withResponse) {
  const entry = fissionBluetoothConnections.get(connectionId);
  if (!entry) return Promise.reject(new Error(`Bluetooth connection ${connectionId} is not known`));
  return entry.server.getPrimaryService(serviceUuid)
    .then((service) => service.getCharacteristic(characteristicUuid))
    .then((characteristic) => withResponse && characteristic.writeValueWithResponse
      ? characteristic.writeValueWithResponse(value)
      : characteristic.writeValue(value));
}

export function fissionPasskeyAvailability() {
  if (!navigator.credentials || !("PublicKeyCredential" in globalThis)) {
    return Promise.resolve({ supported: false, secureContext: !!globalThis.isSecureContext, platform: false, conditional: false, crossPlatform: false, reason: "WebAuthn is not available in this browser" });
  }
  return Promise.all([
    PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable ? PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable().catch(() => false) : Promise.resolve(false),
    PublicKeyCredential.isConditionalMediationAvailable ? PublicKeyCredential.isConditionalMediationAvailable().catch(() => false) : Promise.resolve(false),
  ]).then(([platform, conditional]) => ({ supported: !!globalThis.isSecureContext, secureContext: !!globalThis.isSecureContext, platform, conditional, crossPlatform: true, reason: globalThis.isSecureContext ? null : "passkeys require a secure browser context" }));
}

export function fissionPasskeyRegister(requestJson) {
  return (async () => {
    await publicKeyCredential();
    const request = JSON.parse(requestJson);
    const publicKey = {
      rp: webauthnRelyingParty(request),
      user: { id: passkeyBytes(request.user.id), name: request.user.name, displayName: request.user.display_name },
      challenge: passkeyBytes(request.challenge),
      pubKeyCredParams: pubKeyCredentialParameters(request.pub_key_algorithms),
      timeout: request.timeout_ms || undefined,
      attestation: attestation(request.attestation),
      excludeCredentials: (request.exclude_credentials || []).map(credentialDescriptor),
    };
    if (request.authenticator_selection) {
      publicKey.authenticatorSelection = {
        authenticatorAttachment: attachment(request.authenticator_selection.attachment),
        residentKey: request.authenticator_selection.resident_key === "Required" ? "required" : (request.authenticator_selection.resident_key === "Discouraged" ? "discouraged" : "preferred"),
        userVerification: userVerification(request.authenticator_selection.user_verification),
      };
    }
    const credential = await navigator.credentials.create({ publicKey });
    return {
      credentialId: new Uint8Array(credential.rawId),
      rawId: new Uint8Array(credential.rawId),
      clientDataJSON: new Uint8Array(credential.response.clientDataJSON),
      attestationObject: new Uint8Array(credential.response.attestationObject),
      authenticatorAttachment: credential.authenticatorAttachment || null,
      transports: credential.response.getTransports ? credential.response.getTransports() : [],
    };
  })();
}

export function fissionPasskeyAuthenticate(requestJson) {
  return (async () => {
    await publicKeyCredential();
    const request = JSON.parse(requestJson);
    const options = {
      challenge: passkeyBytes(request.challenge),
      rpId: webauthnRpId(request),
      allowCredentials: (request.allow_credentials || []).map(credentialDescriptor),
      userVerification: userVerification(request.user_verification),
      timeout: request.timeout_ms || undefined,
    };
    const credential = await navigator.credentials.get({ publicKey: options, mediation: mediation(request.mediation) });
    return {
      credentialId: new Uint8Array(credential.rawId),
      rawId: new Uint8Array(credential.rawId),
      userHandle: credential.response.userHandle ? new Uint8Array(credential.response.userHandle) : null,
      clientDataJSON: new Uint8Array(credential.response.clientDataJSON),
      authenticatorData: new Uint8Array(credential.response.authenticatorData),
      signature: new Uint8Array(credential.response.signature),
    };
  })();
}
"#)]
extern "C" {
    fn fissionNotificationPermission() -> String;
    #[wasm_bindgen(catch)]
    fn fissionRequestNotificationPermission() -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionShowNotification(
        id: &str,
        title: &str,
        body: &str,
        silent: bool,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionScheduleNotification(
        id: &str,
        title: &str,
        body: &str,
        silent: bool,
        delay_ms: f64,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionCancelNotification(id: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionCancelAllNotifications() -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionSetAppBadge(count: JsValue) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionClipboardReadText() -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionClipboardWriteText(text: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionGeolocationPermission() -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionCurrentPosition(
        high_accuracy: bool,
        timeout_ms: f64,
        maximum_age_ms: f64,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionVibrate(pattern: JsValue) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionMediaAvailability(kind: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionRequestMedia(kind: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionCapturePhoto(
        facing: &str,
        width: u32,
        height: u32,
        mime_type: &str,
        quality: u8,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionSetCameraTorch(enabled: bool) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionCaptureAudio(duration_ms: u32) -> Result<Promise, JsValue>;
    fn fissionBarcodeAvailable() -> bool;
    #[wasm_bindgen(catch)]
    fn fissionBarcodeDecode(
        bytes: &Uint8Array,
        content_type: &str,
        formats: &Array,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionBarcodeScan(
        formats: &Array,
        timeout_ms: f64,
        allow_multiple: bool,
    ) -> Result<Promise, JsValue>;
    fn fissionNfcAvailability() -> JsValue;
    #[wasm_bindgen(catch)]
    fn fissionNfcScan(timeout_ms: f64) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionNfcWrite(records_json: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionNfcCancel() -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionBluetoothAvailability() -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionBluetoothScan(services: &Array) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionBluetoothConnect(device_id: &str, services: &Array) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionBluetoothDisconnect(connection_id: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionBluetoothRead(
        connection_id: &str,
        service_uuid: &str,
        characteristic_uuid: &str,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionBluetoothWrite(
        connection_id: &str,
        service_uuid: &str,
        characteristic_uuid: &str,
        value: &Uint8Array,
        with_response: bool,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionPasskeyAvailability() -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionPasskeyRegister(request_json: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn fissionPasskeyAuthenticate(request_json: &str) -> Result<Promise, JsValue>;
}

pub(crate) fn register_web_operation_capabilities(async_registry: &mut AsyncRegistry) {
    register_notifications(async_registry);
    register_clipboard(async_registry);
    register_geolocation(async_registry);
    register_haptics(async_registry);
    register_camera(async_registry);
    register_microphone(async_registry);
    register_barcode(async_registry);
    register_nfc(async_registry);
    register_bluetooth(async_registry);
    register_passkeys(async_registry);
    register_unsupported_web_gaps(async_registry);
}

fn register_notifications(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(GET_NOTIFICATION_SETTINGS, move |(), _| async {
        Ok(notification_settings_from_permission(
            &fissionNotificationPermission(),
        ))
    });

    async_registry.register_operation_capability(
        REQUEST_NOTIFICATION_PERMISSION,
        move |_request: NotificationPermissionRequest, _| async {
            let value = await_promise(fissionRequestNotificationPermission())
                .await
                .map_err(notification_error)?;
            Ok(notification_settings_from_permission(
                &value.as_string().unwrap_or_default(),
            ))
        },
    );

    async_registry.register_operation_capability(SHOW_NOTIFICATION, move |request, _| async move {
        if !matches!(request.schedule, NotificationSchedule::Immediate) {
            return Err(NotificationError::unsupported("schedule"));
        }
        let silent = matches!(request.sound, fission_core::NotificationSound::Silent);
        await_promise(fissionShowNotification(
            &request.id.0,
            &request.title,
            &request.body,
            silent,
        ))
        .await
        .map_err(notification_error)?;
        Ok(NotificationReceipt {
            id: request.id,
            scheduled: false,
            delivered: true,
        })
    });

    async_registry.register_operation_capability(
        SCHEDULE_NOTIFICATION,
        move |request, _| async move {
            if matches!(request.schedule, NotificationSchedule::Immediate) {
                let silent = matches!(request.sound, fission_core::NotificationSound::Silent);
                await_promise(fissionShowNotification(
                    &request.id.0,
                    &request.title,
                    &request.body,
                    silent,
                ))
                .await
                .map_err(notification_error)?;
                Ok(NotificationReceipt {
                    id: request.id,
                    scheduled: false,
                    delivered: true,
                })
            } else {
                let delay_ms = notification_delay_ms(&request.schedule);
                let silent = matches!(request.sound, fission_core::NotificationSound::Silent);
                await_promise(fissionScheduleNotification(
                    &request.id.0,
                    &request.title,
                    &request.body,
                    silent,
                    delay_ms as f64,
                ))
                .await
                .map_err(notification_error)?;
                Ok(NotificationReceipt {
                    id: request.id,
                    scheduled: true,
                    delivered: false,
                })
            }
        },
    );

    async_registry.register_operation_capability(
        CANCEL_NOTIFICATION,
        move |request, _| async move {
            await_promise(fissionCancelNotification(&request.id.0))
                .await
                .map_err(notification_error)?;
            Ok(())
        },
    );
    async_registry.register_operation_capability(CANCEL_ALL_NOTIFICATIONS, move |(), _| async {
        await_promise(fissionCancelAllNotifications())
            .await
            .map_err(notification_error)?;
        Ok(())
    });
    async_registry.register_operation_capability(SET_BADGE_COUNT, move |request, _| async move {
        let count = request
            .count
            .map(|count| JsValue::from_f64(count as f64))
            .unwrap_or(JsValue::NULL);
        await_promise(fissionSetAppBadge(count))
            .await
            .map_err(notification_error)?;
        Ok(())
    });
    async_registry.register_operation_capability(
        REGISTER_PUSH_NOTIFICATIONS,
        move |_request: PushRegistrationRequest, _| async {
            Err::<PushRegistration, _>(NotificationError::unsupported("register_push"))
        },
    );
    async_registry
        .register_operation_capability(UNREGISTER_PUSH_NOTIFICATIONS, move |(), _| async {
            Err::<(), _>(NotificationError::unsupported("unregister_push"))
        });
}

fn notification_delay_ms(schedule: &NotificationSchedule) -> u64 {
    match schedule {
        NotificationSchedule::Immediate => 0,
        NotificationSchedule::AfterMillis(ms) => *ms,
        NotificationSchedule::AtUnixMillis(ms) => {
            let now_ms = js_sys::Date::now().max(0.0) as u64;
            ms.saturating_sub(now_ms)
        }
    }
}

fn register_clipboard(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(READ_CLIPBOARD_TEXT, move |(), _| async {
        let value = await_promise(fissionClipboardReadText())
            .await
            .map_err(clipboard_error)?;
        Ok(ClipboardText {
            text: value.as_string(),
        })
    });

    async_registry.register_operation_capability(
        WRITE_CLIPBOARD_TEXT,
        move |request, _| async move {
            await_promise(fissionClipboardWriteText(&request.text))
                .await
                .map_err(clipboard_error)?;
            Ok(())
        },
    );

    async_registry.register_operation_capability(READ_CLIPBOARD_CONTENT, move |(), _| async {
        let value = await_promise(fissionClipboardReadText())
            .await
            .map_err(clipboard_error)?;
        let text = value.as_string().unwrap_or_default();
        Ok(ClipboardContent {
            items: if text.is_empty() {
                Vec::new()
            } else {
                vec![ClipboardItem {
                    content_type: "text/plain".into(),
                    bytes: text.into_bytes(),
                    suggested_name: None,
                }]
            },
        })
    });

    async_registry.register_operation_capability(
        WRITE_CLIPBOARD_CONTENT,
        move |request, _| async move {
            let text = request
                .items
                .iter()
                .find(|item| item.content_type.starts_with("text/plain"))
                .and_then(|item| String::from_utf8(item.bytes.clone()).ok())
                .ok_or_else(|| ClipboardError::unsupported("write_content_non_text"))?;
            await_promise(fissionClipboardWriteText(&text))
                .await
                .map_err(clipboard_error)?;
            Ok(())
        },
    );

    async_registry.register_operation_capability(CLEAR_CLIPBOARD, move |(), _| async {
        await_promise(fissionClipboardWriteText(""))
            .await
            .map_err(clipboard_error)?;
        Ok(())
    });
}

fn register_geolocation(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(GET_GEOLOCATION_PERMISSION, move |(), _| async {
        let value = await_promise(fissionGeolocationPermission())
            .await
            .map_err(geolocation_error)?;
        Ok(geolocation_permission(
            &value.as_string().unwrap_or_default(),
        ))
    });
    async_registry.register_operation_capability(
        REQUEST_GEOLOCATION_PERMISSION,
        move |_request: GeolocationPermissionRequest, _| async {
            let value = await_promise(fissionGeolocationPermission())
                .await
                .map_err(geolocation_error)?;
            Ok(geolocation_permission(
                &value.as_string().unwrap_or_default(),
            ))
        },
    );
    async_registry.register_operation_capability(
        GET_CURRENT_POSITION,
        move |request, _| async move {
            let value = await_promise(fissionCurrentPosition(
                request.high_accuracy,
                request.timeout_ms.map(|value| value as f64).unwrap_or(-1.0),
                request
                    .maximum_age_ms
                    .map(|value| value as f64)
                    .unwrap_or(-1.0),
            ))
            .await
            .map_err(geolocation_error)?;
            Ok(GeolocationPosition {
                latitude: f64_prop(&value, "latitude").unwrap_or_default(),
                longitude: f64_prop(&value, "longitude").unwrap_or_default(),
                altitude_meters: f64_prop(&value, "altitude"),
                accuracy_meters: f64_prop(&value, "accuracy").unwrap_or_default(),
                altitude_accuracy_meters: f64_prop(&value, "altitudeAccuracy"),
                heading_degrees: f64_prop(&value, "heading"),
                speed_mps: f64_prop(&value, "speed"),
                timestamp_unix_ms: f64_prop(&value, "timestamp").unwrap_or_default() as u64,
            })
        },
    );
}

fn register_haptics(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(HAPTIC_SELECTION, move |(), _| async {
        vibrate(vec![10]).await
    });
    async_registry.register_operation_capability(HAPTIC_IMPACT, move |request, _| async move {
        let duration = match request.style {
            HapticImpactStyle::Light | HapticImpactStyle::Soft => 15,
            HapticImpactStyle::Medium => 30,
            HapticImpactStyle::Heavy | HapticImpactStyle::Rigid => 50,
        };
        vibrate(vec![duration]).await
    });
    async_registry.register_operation_capability(
        HAPTIC_NOTIFICATION,
        move |request, _| async move {
            let pattern = match request.kind {
                fission_core::HapticNotificationKind::Success => vec![20, 40, 20],
                fission_core::HapticNotificationKind::Warning => vec![35, 50, 35],
                fission_core::HapticNotificationKind::Error => vec![50, 40, 50, 40, 50],
            };
            vibrate(pattern).await
        },
    );
    async_registry.register_operation_capability(HAPTIC_PATTERN, move |request, _| async move {
        vibrate(haptic_pattern(request.steps)).await
    });
}

fn register_camera(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(GET_CAMERA_AVAILABILITY, move |(), _| async {
        let value = await_promise(fissionMediaAvailability("videoinput"))
            .await
            .map_err(camera_error)?;
        Ok(CameraAvailability {
            permission: camera_permission(&string_prop(&value, "permission").unwrap_or_default()),
            devices: media_devices(&value)
                .into_iter()
                .map(|device| CameraDevice {
                    id: string_prop(&device, "id").unwrap_or_default(),
                    label: string_prop(&device, "label"),
                    facing: match string_prop(&device, "facing").as_deref() {
                        Some("front") => CameraFacing::Front,
                        Some("back") => CameraFacing::Back,
                        _ => CameraFacing::Unspecified,
                    },
                    has_flashlight: false,
                })
                .collect(),
        })
    });

    async_registry.register_operation_capability(
        REQUEST_CAMERA_PERMISSION,
        move |_request: CameraPermissionRequest, _| async {
            let value = await_promise(fissionRequestMedia("videoinput"))
                .await
                .map_err(camera_error)?;
            Ok(camera_permission(&value.as_string().unwrap_or_default()))
        },
    );

    async_registry.register_operation_capability(CAPTURE_PHOTO, move |request, _| async move {
        let (width, height) = request
            .resolution
            .map(|resolution| (resolution.width, resolution.height))
            .unwrap_or((0, 0));
        let value = await_promise(fissionCapturePhoto(
            camera_facing_str(request.facing),
            width,
            height,
            camera_mime_type(request.format),
            request.quality.unwrap_or(92),
        ))
        .await
        .map_err(camera_error)?;
        let bytes = bytes_prop(&value, "bytes");
        if bytes.is_empty() {
            return Err(CameraError::new(
                "capture_failed",
                "web camera capture returned no bytes",
            ));
        }
        Ok(CameraCapture {
            bytes,
            content_type: string_prop(&value, "contentType").unwrap_or_else(|| "image/png".into()),
            width: f64_prop(&value, "width").unwrap_or_default() as u32,
            height: f64_prop(&value, "height").unwrap_or_default() as u32,
            camera_id: string_prop(&value, "deviceId"),
        })
    });

    async_registry.register_operation_capability(
        SET_CAMERA_FLASHLIGHT,
        move |request: CameraFlashlightRequest, _| async move {
            await_promise(fissionSetCameraTorch(request.enabled))
                .await
                .map_err(camera_error)?;
            Ok(())
        },
    );
    async_registry.register_operation_capability(CANCEL_CAMERA_CAPTURE, move |(), _| async {
        Err::<(), _>(CameraError::unsupported("cancel_capture"))
    });
}

fn register_microphone(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(GET_MICROPHONE_AVAILABILITY, move |(), _| async {
        let value = await_promise(fissionMediaAvailability("audioinput"))
            .await
            .map_err(microphone_error)?;
        Ok(MicrophoneAvailability {
            permission: microphone_permission(
                &string_prop(&value, "permission").unwrap_or_default(),
            ),
            devices: media_devices(&value)
                .into_iter()
                .enumerate()
                .map(|(index, device)| MicrophoneDevice {
                    id: string_prop(&device, "id").unwrap_or_default(),
                    label: string_prop(&device, "label"),
                    is_default: index == 0,
                })
                .collect(),
        })
    });

    async_registry.register_operation_capability(
        REQUEST_MICROPHONE_PERMISSION,
        move |_request: MicrophonePermissionRequest, _| async {
            let value = await_promise(fissionRequestMedia("audioinput"))
                .await
                .map_err(microphone_error)?;
            Ok(microphone_permission(
                &value.as_string().unwrap_or_default(),
            ))
        },
    );

    async_registry.register_operation_capability(
        CAPTURE_MICROPHONE_AUDIO,
        move |request, _| async move {
            let duration = request.duration_ms.min(u32::MAX as u64) as u32;
            let value = await_promise(fissionCaptureAudio(duration))
                .await
                .map_err(microphone_error)?;
            Ok(MicrophoneCapture {
                bytes: bytes_prop(&value, "bytes"),
                content_type: string_prop(&value, "contentType")
                    .unwrap_or_else(|| "audio/webm".into()),
                sample_rate_hz: request.sample_rate_hz.unwrap_or(48_000),
                channels: request.channels.unwrap_or(1),
                duration_ms: f64_prop(&value, "durationMs").unwrap_or(duration as f64) as u64,
                device_id: request.device_id,
            })
        },
    );

    async_registry.register_operation_capability(CANCEL_MICROPHONE_CAPTURE, move |(), _| async {
        Err::<(), _>(MicrophoneError::unsupported("cancel_capture"))
    });
}

fn register_barcode(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(SCAN_BARCODE, move |request, _| async move {
        let value = await_promise(fissionBarcodeScan(
            &barcode_format_array(&request.formats),
            request.timeout_ms.map(|value| value as f64).unwrap_or(-1.0),
            request.allow_multiple,
        ))
        .await
        .map_err(barcode_error)?;
        Ok(BarcodeScanResults {
            items: barcode_results(&value),
        })
    });

    async_registry.register_operation_capability(
        DECODE_BARCODE_IMAGE,
        move |request, _| async move {
            let bytes = Uint8Array::from(request.bytes.as_slice());
            let value = await_promise(fissionBarcodeDecode(
                &bytes,
                request.content_type.as_deref().unwrap_or("image/png"),
                &barcode_format_array(&request.formats),
            ))
            .await
            .map_err(barcode_error)?;
            Ok(BarcodeScanResults {
                items: barcode_results(&value),
            })
        },
    );

    async_registry.register_operation_capability(CANCEL_BARCODE_SCAN, move |(), _| async {
        Err::<(), _>(BarcodeScannerError::unsupported("cancel_scan"))
    });
}

fn register_nfc(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(GET_NFC_AVAILABILITY, move |(), _| async {
        let value = fissionNfcAvailability();
        Ok(NfcAvailability {
            supported: bool_prop(&value, "supported").unwrap_or(false),
            enabled: bool_prop(&value, "enabled").unwrap_or(false),
            read: bool_prop(&value, "read").unwrap_or(false),
            write: bool_prop(&value, "write").unwrap_or(false),
            card_emulation: bool_prop(&value, "cardEmulation").unwrap_or(false),
        })
    });

    async_registry.register_operation_capability(SCAN_NFC_TAG, move |request, _| async move {
        let value = await_promise(fissionNfcScan(
            request.timeout_ms.map(|value| value as f64).unwrap_or(-1.0),
        ))
        .await
        .map_err(nfc_error)?;
        Ok(NfcTag {
            id: string_prop(&value, "serialNumber").map(|serial| serial.into_bytes()),
            technologies: vec![NfcTechnology::Ndef],
            records: nfc_records(&value),
            raw_payload: None,
        })
    });

    async_registry.register_operation_capability(WRITE_NFC_TAG, move |request, _| async move {
        let records_json = serde_json::to_string(&request.records)
            .map_err(|error| NfcError::new("serialize_error", error.to_string()))?;
        let value = await_promise(fissionNfcWrite(&records_json))
            .await
            .map_err(nfc_error)?;
        Ok(NfcSessionReceipt {
            session_id: string_prop(&value, "sessionId"),
            completed: bool_prop(&value, "completed").unwrap_or(false),
        })
    });

    async_registry.register_operation_capability(
        EMULATE_NFC_TAG,
        move |_request: NfcEmulationRequest, _| async {
            Err::<NfcSessionReceipt, _>(NfcError::unsupported("emulate_tag"))
        },
    );
    async_registry.register_operation_capability(CANCEL_NFC_SESSION, move |(), _| async {
        await_promise(fissionNfcCancel()).await.map_err(nfc_error)?;
        Ok(())
    });
}

fn register_bluetooth(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(GET_BLUETOOTH_AVAILABILITY, move |(), _| async {
        let value = await_promise(fissionBluetoothAvailability())
            .await
            .map_err(bluetooth_error)?;
        let supported = bool_prop(&value, "supported").unwrap_or(false);
        Ok(BluetoothAvailability {
            permission: if supported {
                BluetoothPermission::Unknown
            } else {
                BluetoothPermission::Denied
            },
            enabled: bool_prop(&value, "enabled").unwrap_or(false),
            supports_classic: false,
            supports_low_energy: supported,
        })
    });

    async_registry.register_operation_capability(
        REQUEST_BLUETOOTH_PERMISSION,
        move |_request: BluetoothPermissionRequest, _| async {
            let value = await_promise(fissionBluetoothAvailability())
                .await
                .map_err(bluetooth_error)?;
            if bool_prop(&value, "supported").unwrap_or(false) {
                Ok(BluetoothPermission::Unknown)
            } else {
                Err(BluetoothError::unsupported("request_permission"))
            }
        },
    );

    async_registry.register_operation_capability(
        SCAN_BLUETOOTH_DEVICES,
        move |request, _| async move {
            let value = await_promise(fissionBluetoothScan(&string_array(&request.service_uuids)))
                .await
                .map_err(bluetooth_error)?;
            Ok(BluetoothScanResult {
                devices: bluetooth_devices(&value),
            })
        },
    );

    async_registry.register_operation_capability(
        CONNECT_BLUETOOTH_DEVICE,
        move |request, _| async move {
            let value = await_promise(fissionBluetoothConnect(
                &request.device_id,
                &string_array(&request.service_uuids),
            ))
            .await
            .map_err(bluetooth_error)?;
            Ok(BluetoothConnection {
                connection_id: string_prop(&value, "connectionId").unwrap_or_default(),
                device: prop(&value, "device")
                    .map(|device| bluetooth_device(&device))
                    .unwrap_or_default(),
            })
        },
    );

    async_registry.register_operation_capability(
        DISCONNECT_BLUETOOTH_DEVICE,
        move |request, _| async move {
            await_promise(fissionBluetoothDisconnect(&request.connection_id))
                .await
                .map_err(bluetooth_error)?;
            Ok(())
        },
    );

    async_registry.register_operation_capability(
        READ_BLUETOOTH_CHARACTERISTIC,
        move |request, _| async move {
            let value = await_promise(fissionBluetoothRead(
                &request.connection_id,
                &request.service_uuid,
                &request.characteristic_uuid,
            ))
            .await
            .map_err(bluetooth_error)?;
            Ok(BluetoothReadResult {
                value: Uint8Array::new(&value).to_vec(),
            })
        },
    );

    async_registry.register_operation_capability(
        WRITE_BLUETOOTH_CHARACTERISTIC,
        move |request, _| async move {
            let value = Uint8Array::from(request.value.as_slice());
            await_promise(fissionBluetoothWrite(
                &request.connection_id,
                &request.service_uuid,
                &request.characteristic_uuid,
                &value,
                request.with_response,
            ))
            .await
            .map_err(bluetooth_error)?;
            Ok(())
        },
    );

    async_registry.register_operation_capability(
        START_BLUETOOTH_ADVERTISING,
        move |_request: BluetoothAdvertiseRequest, _| async {
            Err::<BluetoothAdvertiseReceipt, _>(BluetoothError::unsupported("start_advertising"))
        },
    );
    async_registry.register_operation_capability(
        STOP_BLUETOOTH_ADVERTISING,
        move |_request: BluetoothStopAdvertiseRequest, _| async {
            Err::<(), _>(BluetoothError::unsupported("stop_advertising"))
        },
    );
}

fn register_passkeys(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(GET_PASSKEY_AVAILABILITY, move |(), _| async {
        let value = await_promise(fissionPasskeyAvailability())
            .await
            .map_err(passkey_error)?;
        Ok(PasskeyAvailability {
            supported: bool_prop(&value, "supported").unwrap_or(false),
            secure_context: bool_prop(&value, "secureContext").unwrap_or(false),
            platform_authenticator_available: bool_prop(&value, "platform").unwrap_or(false),
            conditional_ui_available: bool_prop(&value, "conditional").unwrap_or(false),
            cross_platform_authenticator_available: bool_prop(&value, "crossPlatform")
                .unwrap_or(false),
            reason: string_prop(&value, "reason"),
        })
    });

    async_registry.register_operation_capability(REGISTER_PASSKEY, move |request, _| async move {
        let request_json = serde_json::to_string(&request)
            .map_err(|error| PasskeyError::new("serialize_error", error.to_string()))?;
        let value = await_promise(fissionPasskeyRegister(&request_json))
            .await
            .map_err(passkey_error)?;
        Ok(PasskeyRegistrationResult {
            credential_id: bytes_prop(&value, "credentialId"),
            raw_id: bytes_prop(&value, "rawId"),
            client_data_json: bytes_prop(&value, "clientDataJSON"),
            attestation_object: bytes_prop(&value, "attestationObject"),
            authenticator_attachment: string_prop(&value, "authenticatorAttachment").and_then(
                |value| match value.as_str() {
                    "platform" => Some(PasskeyAuthenticatorAttachment::Platform),
                    "cross-platform" => Some(PasskeyAuthenticatorAttachment::CrossPlatform),
                    _ => None,
                },
            ),
            transports: string_vec_prop(&value, "transports")
                .into_iter()
                .map(passkey_transport)
                .collect(),
        })
    });

    async_registry.register_operation_capability(
        AUTHENTICATE_PASSKEY,
        move |request, _| async move {
            let request_json = serde_json::to_string(&request)
                .map_err(|error| PasskeyError::new("serialize_error", error.to_string()))?;
            let value = await_promise(fissionPasskeyAuthenticate(&request_json))
                .await
                .map_err(passkey_error)?;
            let user_handle = prop(&value, "userHandle").and_then(|value| {
                if value.is_null() || value.is_undefined() {
                    None
                } else {
                    Some(Uint8Array::new(&value).to_vec())
                }
            });
            Ok(PasskeyAuthenticationResult {
                credential_id: bytes_prop(&value, "credentialId"),
                raw_id: bytes_prop(&value, "rawId"),
                user_handle,
                client_data_json: bytes_prop(&value, "clientDataJSON"),
                authenticator_data: bytes_prop(&value, "authenticatorData"),
                signature: bytes_prop(&value, "signature"),
            })
        },
    );

    async_registry.register_operation_capability(CANCEL_PASSKEY_OPERATION, move |(), _| async {
        Err::<(), _>(PasskeyError::unsupported("cancel"))
    });
}

fn register_unsupported_web_gaps(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(GET_BIOMETRIC_AVAILABILITY, move |(), _| async {
        Ok(BiometricAvailability {
            reason: Some("Standalone biometric authentication is not exposed by the web platform; use passkeys for biometric-backed sign-in.".into()),
            ..Default::default()
        })
    });
    async_registry.register_operation_capability(
        AUTHENTICATE_BIOMETRIC,
        move |_request, _| async {
            Err::<fission_core::BiometricAuthenticateResult, _>(BiometricError::unsupported(
                "authenticate",
            ))
        },
    );
    async_registry
        .register_operation_capability(CANCEL_BIOMETRIC_AUTHENTICATION, move |(), _| async {
            Err::<(), _>(BiometricError::unsupported("cancel_authentication"))
        });
    async_registry.register_operation_capability(GET_WIFI_AVAILABILITY, move |(), _| async {
        Ok(WifiAvailability {
            permission: WifiPermission::Denied,
            enabled: false,
            connected_network: None,
        })
    });
    async_registry.register_operation_capability(
        REQUEST_WIFI_PERMISSION,
        move |_request, _| async {
            Err::<WifiPermission, _>(WifiError::unsupported("request_permission"))
        },
    );
    async_registry.register_operation_capability(SCAN_WIFI_NETWORKS, move |_request, _| async {
        Err::<fission_core::WifiScanResult, _>(WifiError::unsupported("scan_networks"))
    });
    async_registry.register_operation_capability(CONNECT_WIFI_NETWORK, move |_request, _| async {
        Err::<fission_core::WifiConnection, _>(WifiError::unsupported("connect_network"))
    });
    async_registry
        .register_operation_capability(DISCONNECT_WIFI_NETWORK, move |_request, _| async {
            Err::<(), _>(WifiError::unsupported("disconnect_network"))
        });
    async_registry.register_operation_capability(GET_VOLUME_LEVEL, move |stream, _| async move {
        Err::<VolumeLevel, _>(VolumeError::unsupported(format!("get_level({stream:?})")))
    });
    async_registry.register_operation_capability(SET_VOLUME_LEVEL, move |_request, _| async {
        Err::<VolumeLevel, _>(VolumeError::unsupported("set_level"))
    });
    async_registry.register_operation_capability(ADJUST_VOLUME_LEVEL, move |_request, _| async {
        Err::<VolumeLevel, _>(VolumeError::unsupported("adjust_level"))
    });
}

async fn vibrate(pattern: Vec<u32>) -> Result<(), HapticError> {
    let array = Array::new();
    for value in pattern {
        array.push(&JsValue::from_f64(value as f64));
    }
    await_promise(fissionVibrate(array.into()))
        .await
        .map_err(haptic_error)?;
    Ok(())
}

fn haptic_pattern(steps: Vec<HapticPatternStep>) -> Vec<u32> {
    let mut out = Vec::new();
    for (index, step) in steps.into_iter().enumerate() {
        if index > 0 {
            out.push(20);
        }
        out.push(step.duration_ms.min(u32::MAX as u64) as u32);
    }
    out
}

async fn await_promise(result: Result<Promise, JsValue>) -> Result<JsValue, JsValue> {
    let promise = result?;
    JsFuture::from(promise).await
}

fn prop(value: &JsValue, name: &str) -> Option<JsValue> {
    Reflect::get(value, &JsValue::from_str(name))
        .ok()
        .filter(|value| !value.is_undefined())
}

fn string_prop(value: &JsValue, name: &str) -> Option<String> {
    prop(value, name).and_then(|value| value.as_string())
}

fn string_vec_prop(value: &JsValue, name: &str) -> Vec<String> {
    prop(value, name)
        .and_then(|value| value.dyn_into::<Array>().ok())
        .map(|array| array.iter().filter_map(|value| value.as_string()).collect())
        .unwrap_or_default()
}

fn bool_prop(value: &JsValue, name: &str) -> Option<bool> {
    prop(value, name).and_then(|value| value.as_bool())
}

fn f64_prop(value: &JsValue, name: &str) -> Option<f64> {
    prop(value, name).and_then(|value| value.as_f64())
}

fn bytes_prop(value: &JsValue, name: &str) -> Vec<u8> {
    prop(value, name)
        .map(|value| Uint8Array::new(&value).to_vec())
        .unwrap_or_default()
}

fn media_devices(value: &JsValue) -> Vec<JsValue> {
    prop(value, "devices")
        .and_then(|devices| devices.dyn_into::<Array>().ok())
        .map(|array| array.iter().collect())
        .unwrap_or_default()
}

fn string_array(values: &[String]) -> Array {
    let array = Array::new();
    for value in values {
        array.push(&JsValue::from_str(value));
    }
    array
}

fn js_error(value: JsValue) -> (String, String) {
    let code = string_prop(&value, "name")
        .unwrap_or_else(|| "host_error".into())
        .to_ascii_lowercase();
    let message = string_prop(&value, "message")
        .or_else(|| value.as_string())
        .unwrap_or_else(|| format!("{value:?}"));
    (code, message)
}

fn notification_error(value: JsValue) -> NotificationError {
    let (code, message) = js_error(value);
    NotificationError::new(code, message)
}

fn clipboard_error(value: JsValue) -> ClipboardError {
    let (code, message) = js_error(value);
    ClipboardError::new(code, message)
}

fn geolocation_error(value: JsValue) -> GeolocationError {
    let (code, message) = js_error(value);
    GeolocationError::new(code, message)
}

fn haptic_error(value: JsValue) -> HapticError {
    let (code, message) = js_error(value);
    HapticError::new(code, message)
}

fn camera_error(value: JsValue) -> CameraError {
    let (code, message) = js_error(value);
    CameraError::new(code, message)
}

fn microphone_error(value: JsValue) -> MicrophoneError {
    let (code, message) = js_error(value);
    MicrophoneError::new(code, message)
}

fn barcode_error(value: JsValue) -> BarcodeScannerError {
    let (code, message) = js_error(value);
    BarcodeScannerError::new(code, message)
}

fn nfc_error(value: JsValue) -> NfcError {
    let (code, message) = js_error(value);
    NfcError::new(code, message)
}

fn bluetooth_error(value: JsValue) -> BluetoothError {
    let (code, message) = js_error(value);
    BluetoothError::new(code, message)
}

fn passkey_error(value: JsValue) -> PasskeyError {
    let (code, message) = js_error(value);
    PasskeyError::new(code, message)
}

fn notification_settings_from_permission(value: &str) -> NotificationSettings {
    let permission = match value {
        "granted" => NotificationPermission::Granted,
        "denied" => NotificationPermission::Denied,
        "unsupported" => NotificationPermission::Unsupported,
        _ => NotificationPermission::NotDetermined,
    };
    let enabled = matches!(permission, NotificationPermission::Granted);
    NotificationSettings {
        permission,
        alerts: enabled,
        badge: enabled,
        sound: enabled,
        scheduling: enabled,
        push: false,
    }
}

fn geolocation_permission(value: &str) -> GeolocationPermission {
    match value {
        "granted" => GeolocationPermission::Granted,
        "denied" => GeolocationPermission::Denied,
        "prompt" => GeolocationPermission::Prompt,
        "unsupported" => GeolocationPermission::Unsupported,
        _ => GeolocationPermission::Unknown,
    }
}

fn camera_permission(value: &str) -> CameraPermission {
    match value {
        "granted" => CameraPermission::Granted,
        "denied" => CameraPermission::Denied,
        _ => CameraPermission::Unknown,
    }
}

fn microphone_permission(value: &str) -> MicrophonePermission {
    match value {
        "granted" => MicrophonePermission::Granted,
        "denied" => MicrophonePermission::Denied,
        _ => MicrophonePermission::Unknown,
    }
}

fn camera_facing_str(facing: CameraFacing) -> &'static str {
    match facing {
        CameraFacing::Front => "front",
        CameraFacing::Back => "back",
        _ => "unspecified",
    }
}

fn camera_mime_type(format: CameraImageFormat) -> &'static str {
    match format {
        CameraImageFormat::Jpeg => "image/jpeg",
        CameraImageFormat::Png => "image/png",
        CameraImageFormat::Heif => "image/heif",
        CameraImageFormat::Raw => "image/png",
    }
}

fn barcode_format_array(formats: &[BarcodeFormat]) -> Array {
    let array = Array::new();
    for format in formats {
        let name = match format {
            BarcodeFormat::QrCode => "QrCode",
            BarcodeFormat::Aztec => "Aztec",
            BarcodeFormat::DataMatrix => "DataMatrix",
            BarcodeFormat::Ean13 => "Ean13",
            BarcodeFormat::Ean8 => "Ean8",
            BarcodeFormat::Code128 => "Code128",
            BarcodeFormat::Code39 => "Code39",
            BarcodeFormat::Code93 => "Code93",
            BarcodeFormat::Codabar => "Codabar",
            BarcodeFormat::Itf => "Itf",
            BarcodeFormat::Pdf417 => "Pdf417",
            BarcodeFormat::UpcA => "UpcA",
            BarcodeFormat::UpcE => "UpcE",
            BarcodeFormat::MaxiCode => "MaxiCode",
            BarcodeFormat::Rss14 => "Rss14",
            BarcodeFormat::RssExpanded => "RssExpanded",
            BarcodeFormat::Other(value) => value,
        };
        array.push(&JsValue::from_str(name));
    }
    array
}

fn barcode_results(value: &JsValue) -> Vec<BarcodeScanResult> {
    value
        .dyn_ref::<Array>()
        .map(|array| {
            array
                .iter()
                .map(|item| BarcodeScanResult {
                    value: string_prop(&item, "value").unwrap_or_default(),
                    format: barcode_format(&string_prop(&item, "format").unwrap_or_default()),
                    raw_bytes: bytes_prop(&item, "rawBytes"),
                    bounds: prop(&item, "bounds")
                        .and_then(|value| value.dyn_into::<Array>().ok())
                        .map(|array| {
                            array
                                .iter()
                                .map(|point| BarcodePoint {
                                    x: f64_prop(&point, "x").unwrap_or_default() as i32,
                                    y: f64_prop(&point, "y").unwrap_or_default() as i32,
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    symbology_identifier: string_prop(&item, "symbologyIdentifier"),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn barcode_format(value: &str) -> BarcodeFormat {
    match value {
        "QrCode" => BarcodeFormat::QrCode,
        "Aztec" => BarcodeFormat::Aztec,
        "DataMatrix" => BarcodeFormat::DataMatrix,
        "Ean13" => BarcodeFormat::Ean13,
        "Ean8" => BarcodeFormat::Ean8,
        "Code128" => BarcodeFormat::Code128,
        "Code39" => BarcodeFormat::Code39,
        "Code93" => BarcodeFormat::Code93,
        "Codabar" => BarcodeFormat::Codabar,
        "Itf" => BarcodeFormat::Itf,
        "Pdf417" => BarcodeFormat::Pdf417,
        "UpcA" => BarcodeFormat::UpcA,
        "UpcE" => BarcodeFormat::UpcE,
        "MaxiCode" => BarcodeFormat::MaxiCode,
        "Rss14" => BarcodeFormat::Rss14,
        "RssExpanded" => BarcodeFormat::RssExpanded,
        other => BarcodeFormat::Other(other.into()),
    }
}

fn nfc_records(value: &JsValue) -> Vec<NfcRecord> {
    prop(value, "records")
        .and_then(|records| records.dyn_into::<Array>().ok())
        .map(|array| {
            array
                .iter()
                .map(|record| NfcRecord {
                    type_name_format: match string_prop(&record, "recordType").as_deref() {
                        Some("text") | Some("url") => NfcRecordTypeNameFormat::WellKnown,
                        Some("mime") => NfcRecordTypeNameFormat::MimeMedia,
                        _ => NfcRecordTypeNameFormat::Unknown,
                    },
                    type_name: string_prop(&record, "mediaType")
                        .unwrap_or_default()
                        .into_bytes(),
                    id: bytes_prop(&record, "id"),
                    payload: bytes_prop(&record, "data"),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn bluetooth_devices(value: &JsValue) -> Vec<BluetoothDevice> {
    value
        .dyn_ref::<Array>()
        .map(|array| array.iter().map(|value| bluetooth_device(&value)).collect())
        .unwrap_or_default()
}

fn bluetooth_device(value: &JsValue) -> BluetoothDevice {
    BluetoothDevice {
        id: string_prop(value, "id").unwrap_or_default(),
        name: string_prop(value, "name"),
        address: string_prop(value, "address"),
        rssi: f64_prop(value, "rssi").map(|value| value as i16),
        paired: bool_prop(value, "paired").unwrap_or(false),
        modes: vec![BluetoothMode::LowEnergy],
    }
}

fn passkey_transport(value: String) -> PasskeyTransport {
    match value.as_str() {
        "usb" => PasskeyTransport::Usb,
        "nfc" => PasskeyTransport::Nfc,
        "ble" => PasskeyTransport::Ble,
        "internal" => PasskeyTransport::Internal,
        "hybrid" => PasskeyTransport::Hybrid,
        _ => PasskeyTransport::Unknown,
    }
}
