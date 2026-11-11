package rs.fission.runtime;

import android.Manifest;
import android.app.Activity;
import android.app.Fragment;
import android.content.ClipData;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothManager;
import android.bluetooth.le.BluetoothLeScanner;
import android.bluetooth.le.ScanCallback;
import android.bluetooth.le.ScanFilter;
import android.bluetooth.le.ScanResult;
import android.bluetooth.le.ScanSettings;
import android.content.pm.PackageManager;
import android.print.PageRange;
import android.print.PrintAttributes;
import android.print.PrintDocumentAdapter;
import android.print.PrintDocumentInfo;
import android.print.PrintManager;
import android.graphics.ImageFormat;
import android.hardware.camera2.CameraAccessException;
import android.hardware.camera2.CameraCaptureSession;
import android.hardware.camera2.CameraCharacteristics;
import android.hardware.camera2.CameraDevice;
import android.hardware.camera2.CameraManager;
import android.hardware.camera2.CaptureRequest;
import android.location.Location;
import android.location.LocationListener;
import android.location.LocationManager;
import android.hardware.biometrics.BiometricPrompt;
import android.media.Image;
import android.media.ImageReader;
import android.os.Build;
import android.os.Bundle;
import android.os.CancellationSignal;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.ParcelFileDescriptor;
import android.os.ParcelUuid;
import android.provider.OpenableColumns;
import android.util.Size;
import android.view.Surface;

import java.nio.ByteBuffer;
import java.io.FileOutputStream;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

public final class FissionAndroidCapabilities {
    private FissionAndroidCapabilities() {}

    private static final int PICK_FILE = 0x46f6;

    private static native void nativeFilePickerResult(
            long requestId,
            String[] uris,
            String[] names,
            String[] mimeTypes,
            String[] sizes,
            String errorCode,
            String errorMessage);

    public static void beginPickFile(
            Activity activity,
            long requestId,
            boolean save,
            boolean allowMultiple,
            String suggestedName,
            String[] mimeTypes) {
        activity.runOnUiThread(() -> {
            try {
                FilePickerFragment fragment = new FilePickerFragment();
                Bundle arguments = new Bundle();
                arguments.putLong("requestId", requestId);
                arguments.putBoolean("save", save);
                arguments.putBoolean("allowMultiple", allowMultiple);
                arguments.putString("suggestedName", suggestedName == null ? "" : suggestedName);
                arguments.putStringArray("mimeTypes", mimeTypes == null ? new String[0] : mimeTypes);
                fragment.setArguments(arguments);
                activity.getFragmentManager()
                        .beginTransaction()
                        .add(fragment, "fission-file-picker-" + requestId)
                        .commitAllowingStateLoss();
            } catch (Throwable error) {
                nativeFilePickerResult(
                        requestId, new String[0], new String[0], new String[0], new String[0],
                        "picker_failed", message(error));
            }
        });
    }

    public static int openPickedFileRead(Activity activity, String uriValue) throws Exception {
        return openPickedFile(activity, uriValue, "r");
    }

    public static int openPickedFileWrite(Activity activity, String uriValue) throws Exception {
        return openPickedFile(activity, uriValue, "wt");
    }

    private static int openPickedFile(Activity activity, String uriValue, String mode) throws Exception {
        ParcelFileDescriptor descriptor = activity.getContentResolver()
                .openFileDescriptor(Uri.parse(uriValue), mode);
        if (descriptor == null) throw new IllegalStateException("the document provider returned no file descriptor");
        return descriptor.detachFd();
    }

    private static String message(Throwable error) {
        String value = error.getMessage();
        return value == null || value.isEmpty() ? error.getClass().getSimpleName() : value;
    }

    private static String[] metadata(Activity activity, Uri uri) {
        String name = uri.getLastPathSegment() == null ? "selected-file" : uri.getLastPathSegment();
        String size = "";
        try (Cursor cursor = activity.getContentResolver().query(
                uri,
                new String[] { OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE },
                null,
                null,
                null)) {
            if (cursor != null && cursor.moveToFirst()) {
                int nameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                int sizeIndex = cursor.getColumnIndex(OpenableColumns.SIZE);
                if (nameIndex >= 0 && !cursor.isNull(nameIndex)) name = cursor.getString(nameIndex);
                if (sizeIndex >= 0 && !cursor.isNull(sizeIndex)) size = Long.toString(cursor.getLong(sizeIndex));
            }
        } catch (Throwable ignored) {
            // Some document providers omit metadata; the URI is still readable.
        }
        String mimeType = activity.getContentResolver().getType(uri);
        return new String[] { uri.toString(), name, mimeType == null ? "" : mimeType, size };
    }

    public static final class FilePickerFragment extends Fragment {
        @Override
        public void onCreate(Bundle savedInstanceState) {
            super.onCreate(savedInstanceState);
            if (savedInstanceState != null) return;
            Bundle arguments = getArguments();
            long requestId = arguments == null ? 0L : arguments.getLong("requestId");
            try {
                boolean save = arguments != null && arguments.getBoolean("save");
                boolean allowMultiple = arguments != null && arguments.getBoolean("allowMultiple");
                String suggestedName = arguments == null ? "" : arguments.getString("suggestedName", "");
                String[] mimeTypes = arguments == null ? new String[0] : arguments.getStringArray("mimeTypes");
                Intent intent = new Intent(save ? Intent.ACTION_CREATE_DOCUMENT : Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                String type = mimeTypes == null || mimeTypes.length == 0 ? "*/*" : mimeTypes[0];
                if (mimeTypes != null && mimeTypes.length > 1) type = "*/*";
                intent.setType(type);
                if (mimeTypes != null && mimeTypes.length > 1) {
                    intent.putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes);
                }
                if (save) {
                    if (!suggestedName.isEmpty()) intent.putExtra(Intent.EXTRA_TITLE, suggestedName);
                } else {
                    intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, allowMultiple);
                }
                int flags = Intent.FLAG_GRANT_READ_URI_PERMISSION;
                if (save) flags |= Intent.FLAG_GRANT_WRITE_URI_PERMISSION;
                flags |= Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION;
                intent.addFlags(flags);
                startActivityForResult(intent, PICK_FILE);
            } catch (Throwable error) {
                nativeFilePickerResult(
                        requestId, new String[0], new String[0], new String[0], new String[0],
                        "picker_failed", message(error));
                removeSelf();
            }
        }

        @Override
        public void onActivityResult(int requestCode, int resultCode, Intent data) {
            super.onActivityResult(requestCode, resultCode, data);
            if (requestCode != PICK_FILE) return;
            Bundle arguments = getArguments();
            long requestId = arguments == null ? 0L : arguments.getLong("requestId");
            try {
                if (resultCode != Activity.RESULT_OK || data == null) {
                    nativeFilePickerResult(
                            requestId, new String[0], new String[0], new String[0], new String[0], null, null);
                    return;
                }
                List<Uri> selected = new ArrayList<>();
                ClipData clip = data.getClipData();
                if (clip != null) {
                    for (int index = 0; index < clip.getItemCount(); index++) {
                        Uri uri = clip.getItemAt(index).getUri();
                        if (uri != null && !selected.contains(uri)) selected.add(uri);
                    }
                }
                Uri single = data.getData();
                if (single != null && !selected.contains(single)) selected.add(single);
                if (selected.isEmpty()) {
                    nativeFilePickerResult(
                            requestId, new String[0], new String[0], new String[0], new String[0], null, null);
                    return;
                }
                if (arguments != null && arguments.getBoolean("save")) {
                    int flags = data.getFlags()
                            & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
                    if ((flags & Intent.FLAG_GRANT_WRITE_URI_PERMISSION) == 0) {
                        throw new SecurityException("Android did not grant write access to the selected document");
                    }
                    getActivity().getContentResolver().takePersistableUriPermission(selected.get(0), flags);
                }
                String[] uris = new String[selected.size()];
                String[] names = new String[selected.size()];
                String[] mimeTypes = new String[selected.size()];
                String[] sizes = new String[selected.size()];
                for (int index = 0; index < selected.size(); index++) {
                    String[] row = metadata(getActivity(), selected.get(index));
                    uris[index] = row[0];
                    names[index] = row[1];
                    mimeTypes[index] = row[2];
                    sizes[index] = row[3];
                }
                nativeFilePickerResult(requestId, uris, names, mimeTypes, sizes, null, null);
            } catch (Throwable error) {
                nativeFilePickerResult(
                        requestId, new String[0], new String[0], new String[0], new String[0],
                        error instanceof SecurityException ? "permission_denied" : "picker_failed",
                        message(error));
            } finally {
                removeSelf();
            }
        }

        private void removeSelf() {
            if (getActivity() != null) {
                getActivity().getFragmentManager().beginTransaction().remove(this).commitAllowingStateLoss();
            }
        }
    }

    public static String[] scanBluetoothDevices(
            Activity activity,
            String[] serviceUuids,
            boolean includePaired,
            boolean allowDuplicates,
            long timeoutMillis) throws Exception {
        ensureBluetoothScanPermission(activity);
        BluetoothManager manager = (BluetoothManager) activity.getSystemService(Activity.BLUETOOTH_SERVICE);
        if (manager == null) {
            throw new IllegalStateException("Bluetooth service is not available");
        }
        BluetoothAdapter adapter = manager.getAdapter();
        if (adapter == null || !adapter.isEnabled()) {
            return new String[0];
        }
        Map<String, String> rows = new LinkedHashMap<>();
        if (includePaired) {
            try {
                for (BluetoothDevice device : adapter.getBondedDevices()) {
                    putBluetoothDevice(rows, device, 0, true, true);
                }
            } catch (SecurityException ignored) {
            }
        }
        BluetoothLeScanner scanner = adapter.getBluetoothLeScanner();
        if (scanner == null) {
            return rows.values().toArray(new String[0]);
        }
        List<ScanFilter> filters = new ArrayList<>();
        if (serviceUuids != null) {
            for (String uuid : serviceUuids) {
                try {
                    filters.add(new ScanFilter.Builder().setServiceUuid(new ParcelUuid(UUID.fromString(uuid))).build());
                } catch (Throwable ignored) {
                }
            }
        }
        ScanSettings settings = new ScanSettings.Builder()
                .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
                .build();
        CountDownLatch done = new CountDownLatch(1);
        long timeout = timeoutMillis > 0 ? timeoutMillis : 3000L;
        ScanCallback callback = new ScanCallback() {
            @Override public void onScanResult(int callbackType, ScanResult result) {
                if (result != null && result.getDevice() != null) {
                    putBluetoothDevice(rows, result.getDevice(), result.getRssi(), false, true);
                    if (!allowDuplicates) {
                        rows.put(result.getDevice().getAddress(), rows.get(result.getDevice().getAddress()));
                    }
                }
            }

            @Override public void onBatchScanResults(List<ScanResult> results) {
                if (results == null) {
                    return;
                }
                for (ScanResult result : results) {
                    onScanResult(0, result);
                }
            }

            @Override public void onScanFailed(int errorCode) {
                done.countDown();
            }
        };
        scanner.startScan(filters, settings, callback);
        done.await(timeout, TimeUnit.MILLISECONDS);
        try {
            scanner.stopScan(callback);
        } catch (Throwable ignored) {
        }
        return rows.values().toArray(new String[0]);
    }

    public static double[] currentLocation(
            Activity activity,
            boolean highAccuracy,
            long timeoutMillis) {
        if (activity.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) != PackageManager.PERMISSION_GRANTED
                && activity.checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION) != PackageManager.PERMISSION_GRANTED) {
            return unavailableLocation();
        }
        LocationManager manager = (LocationManager) activity.getSystemService(Activity.LOCATION_SERVICE);
        if (manager == null) {
            return unavailableLocation();
        }
        String[] providers = highAccuracy
                ? new String[] { LocationManager.GPS_PROVIDER, LocationManager.NETWORK_PROVIDER, LocationManager.PASSIVE_PROVIDER }
                : new String[] { LocationManager.NETWORK_PROVIDER, LocationManager.GPS_PROVIDER, LocationManager.PASSIVE_PROVIDER };
        Location last = latestKnownLocation(manager, providers);
        if (last != null) {
            return encodeLocation(last);
        }

        HandlerThread thread = new HandlerThread("FissionLocation");
        thread.start();
        CountDownLatch done = new CountDownLatch(1);
        AtomicReference<Location> result = new AtomicReference<>();
        AtomicReference<Throwable> error = new AtomicReference<>();
        LocationListener listener = new LocationListener() {
            @Override public void onLocationChanged(Location location) {
                if (location != null) {
                    result.set(location);
                }
                done.countDown();
            }

            @Override public void onProviderDisabled(String provider) {
            }

            @Override public void onProviderEnabled(String provider) {
            }

            @Override public void onStatusChanged(String provider, int status, Bundle extras) {
            }
        };
        try {
            boolean requested = false;
            for (String provider : providers) {
                try {
                    if (manager.isProviderEnabled(provider)) {
                        manager.requestLocationUpdates(provider, 0L, 0.0f, listener, thread.getLooper());
                        requested = true;
                    }
                } catch (Throwable throwable) {
                    error.compareAndSet(null, throwable);
                }
            }
            if (!requested) {
                return unavailableLocation();
            }
            long timeout = timeoutMillis > 0 ? timeoutMillis : 5000L;
            done.await(timeout, TimeUnit.MILLISECONDS);
            Location location = result.get();
            if (location == null) {
                location = latestKnownLocation(manager, providers);
            }
            if (location == null) {
                return unavailableLocation();
            }
            return encodeLocation(location);
        } catch (Throwable ignored) {
            return unavailableLocation();
        } finally {
            try {
                manager.removeUpdates(listener);
            } catch (Throwable ignored) {
            }
            thread.quitSafely();
        }
    }

    private static Location latestKnownLocation(LocationManager manager, String[] providers) {
        Location last = null;
        for (String provider : providers) {
            try {
                Location location = manager.getLastKnownLocation(provider);
                if (location != null && (last == null || location.getTime() > last.getTime())) {
                    last = location;
                }
            } catch (Throwable ignored) {
            }
        }
        return last;
    }

    private static double[] unavailableLocation() {
        return new double[] {
                Double.NaN,
                Double.NaN,
                Double.NaN,
                Double.NaN,
                Double.NaN,
                Double.NaN,
                Double.NaN,
                0.0
        };
    }

    public static String authenticateBiometric(
            Activity activity,
            String title,
            String subtitle,
            String reason,
            boolean allowDeviceCredential,
            long timeoutMillis) {
        if (Build.VERSION.SDK_INT < 28) {
            return "error\u001funsupported\u001fAndroid biometric prompts require API 28 or newer";
        }
        CountDownLatch done = new CountDownLatch(1);
        AtomicReference<String> result = new AtomicReference<>("error\u001ftimeout\u001fBiometric authentication did not finish");
        activity.runOnUiThread(() -> {
            try {
                BiometricPrompt.Builder builder = new BiometricPrompt.Builder(activity)
                        .setTitle(title == null || title.isEmpty() ? "Authenticate" : title)
                        .setDescription(reason == null ? "" : reason);
                if (subtitle != null && !subtitle.isEmpty()) {
                    builder.setSubtitle(subtitle);
                }
                if (Build.VERSION.SDK_INT >= 29 && allowDeviceCredential) {
                    builder.setDeviceCredentialAllowed(true);
                } else {
                    builder.setNegativeButton("Cancel", activity.getMainExecutor(), (dialog, which) -> {
                        result.set("error\u001fcancelled\u001fBiometric authentication was cancelled");
                        done.countDown();
                    });
                }
                BiometricPrompt prompt = builder.build();
                CancellationSignal cancellation = new CancellationSignal();
                Executor executor = activity.getMainExecutor();
                prompt.authenticate(cancellation, executor, new BiometricPrompt.AuthenticationCallback() {
                    @Override
                    public void onAuthenticationSucceeded(BiometricPrompt.AuthenticationResult authenticationResult) {
                        result.set("ok\u001fbiometric");
                        done.countDown();
                    }

                    @Override
                    public void onAuthenticationError(int errorCode, CharSequence errString) {
                        result.set("error\u001f" + errorCode + "\u001f" + cleanField(errString == null ? "Biometric authentication failed" : errString.toString()));
                        done.countDown();
                    }

                    @Override
                    public void onAuthenticationFailed() {
                        result.set("error\u001ffailed\u001fBiometric authentication was not recognised");
                        done.countDown();
                    }
                });
            } catch (Throwable throwable) {
                result.set("error\u001fhost_error\u001f" + cleanField(throwable.toString()));
                done.countDown();
            }
        });
        try {
            long timeout = timeoutMillis > 0 ? timeoutMillis : 30000L;
            done.await(timeout, TimeUnit.MILLISECONDS);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
            return "error\u001finterrupted\u001fBiometric authentication was interrupted";
        }
        return result.get();
    }

    public static byte[] captureJpeg(
            Activity activity,
            String requestedCameraId,
            int requestedFacing,
            int requestedWidth,
            int requestedHeight,
            int jpegQuality,
            int flashMode,
            long timeoutMillis) throws Exception {
        try {
        if (activity.checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
            return new byte[0];
        }
        CameraManager manager = (CameraManager) activity.getSystemService(Activity.CAMERA_SERVICE);
        String cameraId = chooseCamera(manager, requestedCameraId, requestedFacing);
        if (cameraId == null) {
            return new byte[0];
        }
        Size size = chooseJpegSize(manager, cameraId, requestedWidth, requestedHeight);
        ImageReader reader = ImageReader.newInstance(size.getWidth(), size.getHeight(), ImageFormat.JPEG, 1);
        HandlerThread thread = new HandlerThread("FissionCameraCapture");
        thread.start();
        Handler handler = new Handler(thread.getLooper());
        AtomicReference<byte[]> result = new AtomicReference<>();
        AtomicReference<Throwable> error = new AtomicReference<>();
        CountDownLatch opened = new CountDownLatch(1);
        CountDownLatch configured = new CountDownLatch(1);
        CountDownLatch captured = new CountDownLatch(1);
        AtomicReference<CameraDevice> camera = new AtomicReference<>();
        AtomicReference<CameraCaptureSession> session = new AtomicReference<>();
        long timeout = timeoutMillis > 0 ? timeoutMillis : 5000L;
        try {
            reader.setOnImageAvailableListener(imageReader -> {
                try (Image image = imageReader.acquireLatestImage()) {
                    if (image == null) {
                        error.compareAndSet(null, new IllegalStateException("camera returned no image"));
                    } else {
                        ByteBuffer buffer = image.getPlanes()[0].getBuffer();
                        byte[] bytes = new byte[buffer.remaining()];
                        buffer.get(bytes);
                        result.set(bytes);
                    }
                } catch (Throwable throwable) {
                    error.compareAndSet(null, throwable);
                } finally {
                    captured.countDown();
                }
            }, handler);
            manager.openCamera(cameraId, new CameraDevice.StateCallback() {
                @Override public void onOpened(CameraDevice device) {
                    camera.set(device);
                    opened.countDown();
                    try {
                        device.createCaptureSession(Arrays.asList(reader.getSurface()), new CameraCaptureSession.StateCallback() {
                            @Override public void onConfigured(CameraCaptureSession captureSession) {
                                session.set(captureSession);
                                configured.countDown();
                            }
                            @Override public void onConfigureFailed(CameraCaptureSession captureSession) {
                                error.compareAndSet(null, new IllegalStateException("camera capture session failed to configure"));
                                configured.countDown();
                            }
                        }, handler);
                    } catch (Throwable throwable) {
                        error.compareAndSet(null, throwable);
                        configured.countDown();
                    }
                }
                @Override public void onDisconnected(CameraDevice device) {
                    device.close();
                    error.compareAndSet(null, new IllegalStateException("camera disconnected"));
                    opened.countDown();
                }
                @Override public void onError(CameraDevice device, int code) {
                    device.close();
                    error.compareAndSet(null, new IllegalStateException("camera open failed with code " + code));
                    opened.countDown();
                }
            }, handler);
            if (!opened.await(timeout, TimeUnit.MILLISECONDS)) {
                throw new IllegalStateException("timed out opening camera");
            }
            rethrow(error.get());
            if (!configured.await(timeout, TimeUnit.MILLISECONDS)) {
                throw new IllegalStateException("timed out configuring camera");
            }
            rethrow(error.get());
            CameraDevice device = camera.get();
            CameraCaptureSession captureSession = session.get();
            if (device == null || captureSession == null) {
                throw new IllegalStateException("camera did not produce a capture session");
            }
            CaptureRequest.Builder request = device.createCaptureRequest(CameraDevice.TEMPLATE_STILL_CAPTURE);
            Surface surface = reader.getSurface();
            request.addTarget(surface);
            request.set(CaptureRequest.CONTROL_MODE, CaptureRequest.CONTROL_MODE_AUTO);
            request.set(CaptureRequest.CONTROL_AF_MODE, CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_PICTURE);
            if (flashMode == 1) {
                request.set(CaptureRequest.CONTROL_AE_MODE, CaptureRequest.CONTROL_AE_MODE_ON_ALWAYS_FLASH);
            } else if (flashMode == 2) {
                request.set(CaptureRequest.CONTROL_AE_MODE, CaptureRequest.CONTROL_AE_MODE_ON_AUTO_FLASH);
            } else {
                request.set(CaptureRequest.CONTROL_AE_MODE, CaptureRequest.CONTROL_AE_MODE_ON);
            }
            request.set(CaptureRequest.JPEG_QUALITY, (byte)Math.max(1, Math.min(100, jpegQuality)));
            captureSession.capture(request.build(), null, handler);
            if (!captured.await(timeout, TimeUnit.MILLISECONDS)) {
                throw new IllegalStateException("timed out capturing photo");
            }
            rethrow(error.get());
            byte[] bytes = result.get();
            if (bytes == null || bytes.length == 0) {
                throw new IllegalStateException("camera captured an empty image");
            }
            return bytes;
        } finally {
            CameraCaptureSession captureSession = session.get();
            if (captureSession != null) {
                captureSession.close();
            }
            CameraDevice device = camera.get();
            if (device != null) {
                device.close();
            }
            reader.close();
            thread.quitSafely();
        }
        } catch (Throwable throwable) {
            return new byte[0];
        }
    }

    private static String chooseCamera(CameraManager manager, String requestedCameraId, int requestedFacing) throws CameraAccessException {
        if (requestedCameraId != null && !requestedCameraId.isEmpty()) {
            return requestedCameraId;
        }
        String fallback = null;
        for (String id : manager.getCameraIdList()) {
            if (fallback == null) {
                fallback = id;
            }
            CameraCharacteristics characteristics = manager.getCameraCharacteristics(id);
            Integer facing = characteristics.get(CameraCharacteristics.LENS_FACING);
            if (facing != null && requestedFacing >= 0 && facing == requestedFacing) {
                return id;
            }
        }
        return fallback;
    }

    private static Size chooseJpegSize(CameraManager manager, String cameraId, int requestedWidth, int requestedHeight) throws CameraAccessException {
        CameraCharacteristics characteristics = manager.getCameraCharacteristics(cameraId);
        android.hardware.camera2.params.StreamConfigurationMap map = characteristics.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP);
        if (map == null) {
            return new Size(Math.max(640, requestedWidth), Math.max(480, requestedHeight));
        }
        Size[] sizes = map.getOutputSizes(ImageFormat.JPEG);
        if (sizes == null || sizes.length == 0) {
            return new Size(Math.max(640, requestedWidth), Math.max(480, requestedHeight));
        }
        final int targetWidth = requestedWidth > 0 ? requestedWidth : 1280;
        final int targetHeight = requestedHeight > 0 ? requestedHeight : 720;
        return Arrays.stream(sizes)
                .filter(size -> size.getWidth() >= targetWidth && size.getHeight() >= targetHeight)
                .min(Comparator.comparingInt(size -> size.getWidth() * size.getHeight()))
                .orElseGet(() -> Arrays.stream(sizes)
                        .max(Comparator.comparingInt(size -> size.getWidth() * size.getHeight()))
                        .orElse(new Size(targetWidth, targetHeight)));
    }

    private static void rethrow(Throwable throwable) throws Exception {
        if (throwable == null) {
            return;
        }
        if (throwable instanceof Exception) {
            throw (Exception) throwable;
        }
        throw new RuntimeException(throwable);
    }

    private static void ensureBluetoothScanPermission(Activity activity) {
        if (Build.VERSION.SDK_INT >= 31) {
            if (activity.checkSelfPermission(Manifest.permission.BLUETOOTH_SCAN) != PackageManager.PERMISSION_GRANTED
                    || activity.checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) != PackageManager.PERMISSION_GRANTED) {
                throw new SecurityException("Bluetooth scan/connect permission is not granted");
            }
        } else if (activity.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) != PackageManager.PERMISSION_GRANTED
                && activity.checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION) != PackageManager.PERMISSION_GRANTED) {
            throw new SecurityException("Bluetooth scan requires location permission on this Android version");
        }
    }

    private static void putBluetoothDevice(Map<String, String> rows, BluetoothDevice device, int rssi, boolean paired, boolean lowEnergy) {
        String address;
        try {
            address = device.getAddress();
        } catch (SecurityException error) {
            return;
        }
        if (address == null || address.isEmpty()) {
            return;
        }
        String name = "";
        try {
            String maybeName = device.getName();
            if (maybeName != null) {
                name = cleanField(maybeName);
            }
        } catch (SecurityException ignored) {
        }
        String modes = lowEnergy ? "le" : "classic";
        rows.put(address, cleanField(address) + "\u001f" + name + "\u001f" + cleanField(address) + "\u001f" + rssi + "\u001f" + paired + "\u001f" + modes);
    }

    private static String cleanField(String value) {
        return value == null ? "" : value.replace('\u001f', ' ').replace('\n', ' ').replace('\r', ' ');
    }

    private static double[] encodeLocation(Location location) {
        return new double[] {
                location.getLatitude(),
                location.getLongitude(),
                location.hasAltitude() ? location.getAltitude() : Double.NaN,
                location.hasAccuracy() ? location.getAccuracy() : 0.0,
                location.hasVerticalAccuracy() ? location.getVerticalAccuracyMeters() : Double.NaN,
                location.hasBearing() ? location.getBearing() : Double.NaN,
                location.hasSpeed() ? location.getSpeed() : Double.NaN,
                (double) location.getTime()
        };
    }

    public static String printPdf(Activity activity, byte[] pdf, String jobName) throws Exception {
        if (Build.VERSION.SDK_INT < 19) {
            throw new UnsupportedOperationException("Android system printing requires Android 4.4 or newer");
        }
        if (activity == null || pdf == null || pdf.length < 5
                || pdf[0] != '%' || pdf[1] != 'P' || pdf[2] != 'D' || pdf[3] != 'F' || pdf[4] != '-') {
            throw new IllegalArgumentException("Fission printing requires a valid PDF document");
        }

        final String displayName = jobName == null || jobName.trim().isEmpty() ? "Fission document" : jobName.trim();
        final byte[] document = pdf.clone();
        final CountDownLatch started = new CountDownLatch(1);
        final AtomicReference<String> jobId = new AtomicReference<>();
        final AtomicReference<Throwable> failure = new AtomicReference<>();
        activity.runOnUiThread(() -> {
            try {
                PrintManager manager = (PrintManager) activity.getSystemService(Context.PRINT_SERVICE);
                if (manager == null) {
                    throw new UnsupportedOperationException("Android PrintManager is not available");
                }
                PrintDocumentAdapter adapter = new PrintDocumentAdapter() {
                    @Override public void onLayout(
                            PrintAttributes oldAttributes,
                            PrintAttributes newAttributes,
                            CancellationSignal cancellationSignal,
                            LayoutResultCallback callback,
                            Bundle extras) {
                        if (cancellationSignal.isCanceled()) {
                            callback.onLayoutCancelled();
                            return;
                        }
                        PrintDocumentInfo info = new PrintDocumentInfo.Builder(displayName)
                                .setContentType(PrintDocumentInfo.CONTENT_TYPE_DOCUMENT)
                                .setPageCount(PrintDocumentInfo.PAGE_COUNT_UNKNOWN)
                                .build();
                        callback.onLayoutFinished(info, oldAttributes == null || !newAttributes.equals(oldAttributes));
                    }

                    @Override public void onWrite(
                            PageRange[] pages,
                            android.os.ParcelFileDescriptor destination,
                            CancellationSignal cancellationSignal,
                            WriteResultCallback callback) {
                        if (cancellationSignal.isCanceled()) {
                            callback.onWriteCancelled();
                            return;
                        }
                        if (pages == null || pages.length != 1 || !PageRange.ALL_PAGES.equals(pages[0])) {
                            callback.onWriteFailed("This Android host cannot filter the selected PDF page range.");
                            return;
                        }
                        try (FileOutputStream output = new FileOutputStream(destination.getFileDescriptor())) {
                            output.write(document);
                            callback.onWriteFinished(new PageRange[] { PageRange.ALL_PAGES });
                        } catch (Throwable error) {
                            callback.onWriteFailed(error.toString());
                        }
                    }
                };
                android.print.PrintJob job = manager.print(
                        displayName,
                        adapter,
                        new PrintAttributes.Builder().build());
                if (job == null) {
                    throw new IllegalStateException("Android did not create a print job");
                }
                jobId.set(job.getId().flattenToString());
            } catch (Throwable error) {
                failure.set(error);
            } finally {
                started.countDown();
            }
        });
        if (!started.await(30, TimeUnit.SECONDS)) {
            throw new IllegalStateException("Android print sheet did not start in time");
        }
        Throwable error = failure.get();
        if (error != null) {
            if (error instanceof Exception) throw (Exception) error;
            throw new RuntimeException(error);
        }
        return jobId.get();
    }
}
