#include "fission_skia_ganesh_d3d_internal.h"

#if !defined(FISSION_SKIA_ENABLE_GANESH_D3D)
#error "Compile this source only for the Direct3D native-Ganesh profile"
#endif

#include "include/gpu/ganesh/GrTypes.h"
#include "include/gpu/ganesh/d3d/GrD3DDirectContext.h"

#include <chrono>
#include <climits>
#include <new>
#include <utility>

namespace fission::skia::ganesh::d3d {
namespace {

constexpr uint32_t kMaximumEnumeratedAdapters = 128u;
constexpr DWORD kFenceHealthPollMilliseconds = 100u;

bool is_out_of_memory(HRESULT result) {
    return result == E_OUTOFMEMORY ||
           result == HRESULT_FROM_WIN32(ERROR_NOT_ENOUGH_MEMORY) ||
           result == HRESULT_FROM_WIN32(ERROR_OUTOFMEMORY);
}

bool is_device_failure(HRESULT result) {
    return result == DXGI_ERROR_DEVICE_HUNG ||
           result == DXGI_ERROR_DEVICE_REMOVED ||
           result == DXGI_ERROR_DEVICE_RESET ||
           result == DXGI_ERROR_DRIVER_INTERNAL_ERROR;
}

bool is_unsupported(HRESULT result) {
    bool unsupported = result == DXGI_ERROR_UNSUPPORTED ||
                       result == E_NOINTERFACE || result == E_NOTIMPL;
#if defined(D3D12_ERROR_ADAPTER_NOT_FOUND)
    unsupported = unsupported || result == D3D12_ERROR_ADAPTER_NOT_FOUND;
#endif
#if defined(D3D12_ERROR_DRIVER_VERSION_MISMATCH)
    unsupported = unsupported || result == D3D12_ERROR_DRIVER_VERSION_MISMATCH;
#endif
    return unsupported;
}

Result device_lost(D3DContext::Impl& context) {
    context.device_lost.store(true, std::memory_order_release);
    return Result::failure(
        FISSION_SKIA_STATUS_DEVICE_LOST,
        "the Direct3D device was removed or reset");
}

bool adapter_is_hardware(IDXGIAdapter1& adapter) {
    DXGI_ADAPTER_DESC1 description{};
    return SUCCEEDED(adapter.GetDesc1(&description)) &&
           (description.Flags & DXGI_ADAPTER_FLAG_SOFTWARE) == 0;
}

bool try_create_device(
    D3DContext::Impl& context,
    gr_cp<IDXGIAdapter1> adapter,
    bool* saw_out_of_memory) {
    if (!adapter || !adapter_is_hardware(*adapter.get())) return false;

    gr_cp<ID3D12Device> device;
    const HRESULT result = D3D12CreateDevice(
        adapter.get(),
        D3D_FEATURE_LEVEL_11_0,
        IID_PPV_ARGS(&device));
    if (FAILED(result)) {
        if (is_out_of_memory(result)) *saw_out_of_memory = true;
        return false;
    }
    context.adapter = std::move(adapter);
    context.device = std::move(device);
    return true;
}

Result choose_hardware_device(D3DContext::Impl& context) {
    bool saw_out_of_memory = false;
    gr_cp<IDXGIFactory6> factory6;
    if (SUCCEEDED(context.factory->QueryInterface(IID_PPV_ARGS(&factory6)))) {
        for (uint32_t index = 0; index < kMaximumEnumeratedAdapters; ++index) {
            gr_cp<IDXGIAdapter1> adapter;
            const HRESULT result = factory6->EnumAdapterByGpuPreference(
                index,
                DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
                IID_PPV_ARGS(&adapter));
            if (result == DXGI_ERROR_NOT_FOUND) break;
            if (FAILED(result)) {
                return classify_d3d_result(
                    context,
                    result,
                    FISSION_SKIA_STATUS_UNSUPPORTED,
                    "DXGI could not enumerate hardware adapters");
            }
            if (try_create_device(
                    context, std::move(adapter), &saw_out_of_memory)) {
                return Result::success();
            }
        }
    } else {
        for (uint32_t index = 0; index < kMaximumEnumeratedAdapters; ++index) {
            gr_cp<IDXGIAdapter1> adapter;
            const HRESULT result = context.factory->EnumAdapters1(index, &adapter);
            if (result == DXGI_ERROR_NOT_FOUND) break;
            if (FAILED(result)) {
                return classify_d3d_result(
                    context,
                    result,
                    FISSION_SKIA_STATUS_UNSUPPORTED,
                    "DXGI could not enumerate hardware adapters");
            }
            if (try_create_device(
                    context, std::move(adapter), &saw_out_of_memory)) {
                return Result::success();
            }
        }
    }

    if (saw_out_of_memory) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Direct3D could not allocate a device while enumerating adapters");
    }
    return Result::failure(
        FISSION_SKIA_STATUS_UNSUPPORTED,
        "no hardware adapter supports the required Direct3D 12 feature level");
}

Result create_queue_and_fence(D3DContext::Impl& context) {
    D3D12_COMMAND_QUEUE_DESC queue_description{};
    queue_description.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    queue_description.Priority = D3D12_COMMAND_QUEUE_PRIORITY_NORMAL;
    queue_description.Flags = D3D12_COMMAND_QUEUE_FLAG_NONE;
    queue_description.NodeMask = 0;
    HRESULT result = context.device->CreateCommandQueue(
        &queue_description, IID_PPV_ARGS(&context.queue));
    if (FAILED(result)) {
        return classify_d3d_result(
            context,
            result,
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "Direct3D could not create a graphics command queue");
    }

    result = context.device->CreateFence(
        0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&context.queue_fence));
    if (FAILED(result)) {
        return classify_d3d_result(
            context,
            result,
            FISSION_SKIA_STATUS_INTERNAL,
            "Direct3D could not create the presentation fence");
    }
    context.queue_fence_event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (context.queue_fence_event == nullptr) {
        const DWORD error = GetLastError();
        const fission_skia_status_t status =
            error == ERROR_NOT_ENOUGH_MEMORY || error == ERROR_OUTOFMEMORY
                ? FISSION_SKIA_STATUS_OUT_OF_MEMORY
                : FISSION_SKIA_STATUS_INTERNAL;
        return Result::failure(
            status,
            "Windows could not create the Direct3D presentation event");
    }
    return Result::success();
}

}  // namespace

Result classify_d3d_result(
    D3DContext::Impl& context,
    HRESULT result,
    fission_skia_status_t fallback_status,
    const char* fallback_message) {
    if (SUCCEEDED(result)) return Result::success();
    if (is_out_of_memory(result)) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Direct3D could not allocate a required resource");
    }
    if (is_device_failure(result)) return device_lost(context);

    if (context.device) {
        const HRESULT removed_reason = context.device->GetDeviceRemovedReason();
        if (FAILED(removed_reason)) return device_lost(context);
    }
    if (is_unsupported(result)) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "the Direct3D driver does not support a required capability");
    }
    return Result::failure(fallback_status, fallback_message);
}

Result current_context_health(D3DContext::Impl& context) {
    if (context.device_lost.load(std::memory_order_acquire)) {
        return Result::failure(
            FISSION_SKIA_STATUS_DEVICE_LOST,
            "the Direct3D device is lost");
    }
    if (!context.factory || !context.device || !context.queue || !context.ganesh) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D Ganesh context is not initialized");
    }
    if (FAILED(context.device->GetDeviceRemovedReason()) ||
        context.ganesh->isDeviceLost()) {
        return device_lost(context);
    }
    if (context.ganesh->abandoned()) {
        return Result::failure(
            FISSION_SKIA_STATUS_CONTEXT_LOST,
            "the Direct3D Ganesh context was abandoned");
    }
    return Result::success();
}

Result signal_command_queue(
    D3DContext::Impl& context,
    uint64_t* out_fence_value) {
    if (out_fence_value == nullptr || !context.queue || !context.queue_fence ||
        context.next_queue_fence_value == UINT64_MAX) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D presentation fence cannot be signalled");
    }
    Result health = current_context_health(context);
    if (!health.ok()) return health;

    const uint64_t value = context.next_queue_fence_value++;
    const HRESULT result = context.queue->Signal(context.queue_fence.get(), value);
    if (FAILED(result)) {
        return classify_d3d_result(
            context,
            result,
            FISSION_SKIA_STATUS_INTERNAL,
            "the Direct3D queue could not signal its presentation fence");
    }
    *out_fence_value = value;
    return Result::success();
}

Result wait_for_queue_fence(
    D3DContext::Impl& context,
    uint64_t fence_value) {
    if (fence_value == 0) return Result::success();
    if (!context.queue_fence || context.queue_fence_event == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D presentation fence is not initialized");
    }

    uint64_t completed = context.queue_fence->GetCompletedValue();
    if (completed == UINT64_MAX) return device_lost(context);
    if (completed >= fence_value) return current_context_health(context);

    const HRESULT result = context.queue_fence->SetEventOnCompletion(
        fence_value, context.queue_fence_event);
    if (FAILED(result)) {
        return classify_d3d_result(
            context,
            result,
            FISSION_SKIA_STATUS_INTERNAL,
            "Direct3D could not arm its presentation fence event");
    }

    for (;;) {
        const DWORD wait = WaitForSingleObject(
            context.queue_fence_event, kFenceHealthPollMilliseconds);
        if (wait == WAIT_OBJECT_0) return current_context_health(context);
        if (wait == WAIT_TIMEOUT) {
            Result health = current_context_health(context);
            if (!health.ok()) return health;
            continue;
        }
        const DWORD error = GetLastError();
        const HRESULT wait_result = error == ERROR_SUCCESS
            ? E_FAIL
            : HRESULT_FROM_WIN32(error);
        return classify_d3d_result(
            context,
            wait_result,
            FISSION_SKIA_STATUS_INTERNAL,
            "Windows failed while waiting for Direct3D presentation work");
    }
}

Result drain_command_queue(D3DContext::Impl& context) {
    Result health = current_context_health(context);
    if (!health.ok()) return health;
    uint64_t fence_value = 0;
    Result status = signal_command_queue(context, &fence_value);
    if (!status.ok()) return status;
    return wait_for_queue_fence(context, fence_value);
}

bool valid_windows_window(const WindowsWindow& window) {
    if (window.struct_size != sizeof(window) || window.hwnd == nullptr) {
        return false;
    }
    HWND hwnd = reinterpret_cast<HWND>(const_cast<void*>(window.hwnd));
    if (IsWindow(hwnd) == FALSE) return false;
    DWORD process_id = 0;
    const DWORD thread_id = GetWindowThreadProcessId(hwnd, &process_id);
    return thread_id != 0 && process_id == GetCurrentProcessId() &&
           thread_id == GetCurrentThreadId();
}

D3DContext::D3DContext()
    : impl_(new (std::nothrow) Impl{}) {}

D3DContext::~D3DContext() {
    if (!impl_) return;

    Result health = impl_->ganesh
        ? current_context_health(*impl_)
        : Result::failure(
              FISSION_SKIA_STATUS_INVALID_STATE,
              "the Direct3D Ganesh context is not initialized");
    if (impl_->ganesh && health.ok()) {
        impl_->ganesh->flushAndSubmit(GrSyncCpu::kYes);
        health = drain_command_queue(*impl_);
    }
    if (impl_->ganesh) {
        if (!health.ok() || impl_->ganesh->isDeviceLost()) {
            impl_->device_lost.store(true, std::memory_order_release);
            impl_->ganesh->abandonContext();
        } else {
            impl_->ganesh->releaseResourcesAndAbandonContext();
        }
        impl_->ganesh.reset();
    }
    if (impl_->queue_fence_event != nullptr) {
        CloseHandle(impl_->queue_fence_event);
        impl_->queue_fence_event = nullptr;
    }
    impl_->queue_fence.reset();
    impl_->queue.reset();
    impl_->device.reset();
    impl_->adapter.reset();
    impl_->factory.reset();
}

Result D3DContext::create(
    const WindowsWindow& compatible_window,
    std::unique_ptr<D3DContext>* out_context) {
    if (out_context == nullptr || !valid_windows_window(compatible_window)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Direct3D context output or Windows host window is invalid");
    }
    out_context->reset();
    auto context = std::unique_ptr<D3DContext>(
        new (std::nothrow) D3DContext());
    if (!context || !context->impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the Direct3D Ganesh context state could not be allocated");
    }

    HRESULT result = CreateDXGIFactory2(
        0, IID_PPV_ARGS(&context->impl_->factory));
    if (FAILED(result)) {
        return classify_d3d_result(
            *context->impl_,
            result,
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "DXGI could not create a factory");
    }
    Result status = choose_hardware_device(*context->impl_);
    if (!status.ok()) return status;
    status = create_queue_and_fence(*context->impl_);
    if (!status.ok()) return status;

    GrD3DBackendContext backend;
    backend.fAdapter = context->impl_->adapter;
    backend.fDevice = context->impl_->device;
    backend.fQueue = context->impl_->queue;
    backend.fProtectedContext = GrProtected::kNo;
    context->impl_->ganesh = GrDirectContexts::MakeD3D(backend);
    if (!context->impl_->ganesh) {
        if (FAILED(context->impl_->device->GetDeviceRemovedReason())) {
            return device_lost(*context->impl_);
        }
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "Skia could not create a Ganesh Direct3D context");
    }
    *out_context = std::move(context);
    return Result::success();
}

Result D3DContext::set_resource_cache_limit(uint64_t limit_bytes) {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D Ganesh context is not initialized");
    }
    if (limit_bytes > static_cast<uint64_t>(std::numeric_limits<size_t>::max())) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the GPU resource-cache limit exceeds this platform's address range");
    }
    Result health = current_context_health(*impl_);
    if (!health.ok()) return health;
    impl_->ganesh->setResourceCacheLimit(static_cast<size_t>(limit_bytes));
    return Result::success();
}

Result D3DContext::resource_cache_usage(
    uint64_t* out_resource_count,
    uint64_t* out_resource_bytes) const {
    if (out_resource_count == nullptr || out_resource_bytes == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the GPU resource-cache usage outputs are null");
    }
    *out_resource_count = 0;
    *out_resource_bytes = 0;
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D Ganesh context is not initialized");
    }
    Result health = current_context_health(*impl_);
    if (!health.ok()) return health;

    int resource_count = 0;
    size_t resource_bytes = 0;
    impl_->ganesh->getResourceCacheUsage(&resource_count, &resource_bytes);
    if (resource_count < 0) {
        return Result::failure(
            FISSION_SKIA_STATUS_INTERNAL,
            "Skia reported a negative GPU resource-cache count");
    }
    *out_resource_count = static_cast<uint64_t>(resource_count);
    *out_resource_bytes = static_cast<uint64_t>(resource_bytes);
    return Result::success();
}

Result D3DContext::trim_memory(uint32_t pressure) {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D Ganesh context is not initialized");
    }
    Result health = current_context_health(*impl_);
    if (!health.ok()) return health;

    if (pressure == FISSION_SKIA_MEMORY_PRESSURE_MODERATE) {
        impl_->ganesh->performDeferredCleanup(
            std::chrono::milliseconds(0),
            GrPurgeResourceOptions::kScratchResourcesOnly);
    } else if (pressure == FISSION_SKIA_MEMORY_PRESSURE_CRITICAL) {
        impl_->ganesh->purgeUnlockedResources(
            GrPurgeResourceOptions::kAllResources);
    } else {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the memory-pressure value is unknown");
    }
    impl_->ganesh->checkAsyncWorkCompletion();
    return current_context_health(*impl_);
}

Result D3DContext::health() const {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Direct3D Ganesh context is not initialized");
    }
    return current_context_health(*impl_);
}

bool D3DContext::is_device_lost() const {
    return !impl_ ||
           current_context_health(*impl_).status == FISSION_SKIA_STATUS_DEVICE_LOST;
}

D3DContext::Impl& D3DContext::internal_state() {
    return *impl_;
}

const D3DContext::Impl& D3DContext::internal_state() const {
    return *impl_;
}

}  // namespace fission::skia::ganesh::d3d
