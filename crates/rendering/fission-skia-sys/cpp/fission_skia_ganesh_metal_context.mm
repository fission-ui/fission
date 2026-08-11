#include "fission_skia_ganesh_metal_internal.h"

#if !defined(__APPLE__)
#error "The Fission Ganesh Metal runtime is Apple-only"
#endif

#import <TargetConditionals.h>

#if TARGET_OS_IPHONE
#error "This module implements the macOS AppKit Metal host; iOS uses a separate presenter"
#endif

#if !defined(FISSION_SKIA_ENABLE_GANESH_METAL)
#error "Compile this source only when the Ganesh Metal runtime is enabled"
#endif

#include "include/gpu/ganesh/mtl/GrMtlBackendContext.h"
#include "include/gpu/ganesh/mtl/GrMtlDirectContext.h"

#import <Cocoa/Cocoa.h>
#import <Metal/Metal.h>

#include <chrono>
#include <new>
#include <utility>

namespace fission::skia::ganesh::metal {
namespace {

Result validate_host_window(const MacOSWindow& window) {
    if (!valid_macos_window(window)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the macOS native-view descriptor is invalid");
    }
    if (![NSThread isMainThread]) {
        return Result::failure(
            FISSION_SKIA_STATUS_WRONG_THREAD,
            "macOS native-view operations must run on the main thread");
    }
    id object = (id)window.ns_view;
    if (![object isKindOfClass:[NSView class]]) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the macOS native-view descriptor does not contain an NSView");
    }
    return Result::success();
}

fission_skia_status_t command_buffer_failure(
    id<MTLCommandBuffer> command_buffer) {
    if (command_buffer == nil ||
        command_buffer.status != MTLCommandBufferStatusError) {
        return FISSION_SKIA_STATUS_OK;
    }
    NSError* error = command_buffer.error;
    if (error == nil || ![error.domain isEqualToString:MTLCommandBufferErrorDomain]) {
        return FISSION_SKIA_STATUS_CONTEXT_LOST;
    }
    switch ((MTLCommandBufferError)error.code) {
        case MTLCommandBufferErrorOutOfMemory:
            return FISSION_SKIA_STATUS_OUT_OF_MEMORY;
        case MTLCommandBufferErrorBlacklisted:
        case MTLCommandBufferErrorDeviceRemoved:
            return FISSION_SKIA_STATUS_DEVICE_LOST;
        case MTLCommandBufferErrorNone:
            return FISSION_SKIA_STATUS_OK;
        default:
            return FISSION_SKIA_STATUS_CONTEXT_LOST;
    }
}

Result health_result(fission_skia_status_t status) {
    switch (status) {
        case FISSION_SKIA_STATUS_OK:
            return Result::success();
        case FISSION_SKIA_STATUS_OUT_OF_MEMORY:
            return Result::failure(
                status,
                "Metal exhausted memory while executing a command buffer");
        case FISSION_SKIA_STATUS_DEVICE_LOST:
            return Result::failure(
                status,
                "the Metal device was removed or became unavailable");
        case FISSION_SKIA_STATUS_CONTEXT_LOST:
            return Result::failure(
                status,
                "the Metal command queue or Ganesh context was lost");
        default:
            return Result::failure(
                status,
                "the Metal context recorded an unexpected failure");
    }
}

}  // namespace

bool valid_macos_window(const MacOSWindow& window) {
    return window.struct_size == sizeof(MacOSWindow) &&
           window.ns_view != nullptr;
}

Result current_context_health(MetalContext::Impl& context) {
    if (!context.health) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Metal context has no health state");
    }
    fission_skia_status_t status = context.health->load();
    if (status != FISSION_SKIA_STATUS_OK) return health_result(status);
    if (!context.ganesh) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh Metal context is not initialized");
    }
    if (context.ganesh->isDeviceLost()) {
        context.health->record(FISSION_SKIA_STATUS_DEVICE_LOST);
        return health_result(FISSION_SKIA_STATUS_DEVICE_LOST);
    }
    if (context.ganesh->abandoned()) {
        context.health->record(FISSION_SKIA_STATUS_CONTEXT_LOST);
        return health_result(FISSION_SKIA_STATUS_CONTEXT_LOST);
    }
    if (context.ganesh->oomed()) {
        context.health->record(FISSION_SKIA_STATUS_OUT_OF_MEMORY);
        return health_result(FISSION_SKIA_STATUS_OUT_OF_MEMORY);
    }
    return Result::success();
}

void record_command_buffer_completion(
    const std::shared_ptr<DeviceHealth>& health,
    GrMTLHandle raw_command_buffer) {
    if (!health || raw_command_buffer == nullptr) return;
    id<MTLCommandBuffer> command_buffer =
        (id<MTLCommandBuffer>)raw_command_buffer;
    const fission_skia_status_t failure =
        command_buffer_failure(command_buffer);
    if (failure != FISSION_SKIA_STATUS_OK) health->record(failure);
}

Result drain_command_queue(MetalContext::Impl& context) {
    if (!context.health) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Metal context has no health state");
    }
    Result health = health_result(context.health->load());
    if (!health.ok()) return health;
    id<MTLCommandQueue> queue = (id<MTLCommandQueue>)context.queue.get();
    if (queue == nil) {
        context.health->record(FISSION_SKIA_STATUS_CONTEXT_LOST);
        return health_result(FISSION_SKIA_STATUS_CONTEXT_LOST);
    }

    @autoreleasepool {
        id<MTLCommandBuffer> marker = [queue commandBuffer];
        if (marker == nil) {
            context.health->record(FISSION_SKIA_STATUS_CONTEXT_LOST);
            return health_result(FISSION_SKIA_STATUS_CONTEXT_LOST);
        }
        marker.label = @"Fission Metal lifecycle drain";
        [marker commit];
        [marker waitUntilCompleted];
        record_command_buffer_completion(context.health, (GrMTLHandle)marker);
        health = health_result(context.health->load());
        if (!health.ok()) return health;
    }
    return Result::success();
}

MetalContext::MetalContext()
    : impl_(new (std::nothrow) Impl{}) {}

MetalContext::~MetalContext() {
    if (!impl_) return;

    if (impl_->ganesh) {
        const Result health = current_context_health(*impl_);
        if (health.ok()) {
            if (!impl_->ganesh->submit(GrSyncCpu::kYes)) {
                impl_->health->record(FISSION_SKIA_STATUS_CONTEXT_LOST);
            }
            if (impl_->health->load() == FISSION_SKIA_STATUS_OK) {
                impl_->ganesh->releaseResourcesAndAbandonContext();
            } else {
                impl_->ganesh->abandonContext();
            }
        } else {
            impl_->ganesh->abandonContext();
        }
        impl_->ganesh.reset();
    }

    if (impl_->health && impl_->health->load() == FISSION_SKIA_STATUS_OK &&
        impl_->queue) {
        (void)drain_command_queue(*impl_);
    }
    impl_->queue.reset();
    impl_->device.reset();
    impl_->health.reset();
}

Result MetalContext::create(
    const MacOSWindow& compatible_window,
    std::unique_ptr<MetalContext>* out_context) {
    if (out_context == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh Metal context output is null");
    }
    out_context->reset();
    Result status = validate_host_window(compatible_window);
    if (!status.ok()) return status;

    auto context = std::unique_ptr<MetalContext>(
        new (std::nothrow) MetalContext());
    if (!context || !context->impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the Ganesh Metal context state could not be allocated");
    }
    context->impl_->health = std::make_shared<DeviceHealth>();
    if (!context->impl_->health) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the Metal device health state could not be allocated");
    }

    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (device == nil) {
            return Result::failure(
                FISSION_SKIA_STATUS_UNSUPPORTED,
                "macOS did not provide a Metal device");
        }
        context->impl_->device.reset((GrMTLHandle)device);

        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (queue == nil) {
            return Result::failure(
                FISSION_SKIA_STATUS_OUT_OF_MEMORY,
                "Metal could not create the Fission command queue");
        }
        context->impl_->queue.reset((GrMTLHandle)queue);

        GrMtlBackendContext backend{};
        backend.fDevice.retain(context->impl_->device.get());
        backend.fQueue.retain(context->impl_->queue.get());
        context->impl_->ganesh = GrDirectContexts::MakeMetal(backend);
    }
    if (!context->impl_->ganesh) {
        return Result::failure(
            FISSION_SKIA_STATUS_UNSUPPORTED,
            "Skia could not create a Ganesh Metal context");
    }
    context->impl_->ganesh->setResourceCacheLimit(kDefaultGpuResourceBudget);
    *out_context = std::move(context);
    return Result::success();
}

Result MetalContext::trim_memory(uint32_t pressure) {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh Metal context is not initialized");
    }
    Result status = current_context_health(*impl_);
    if (!status.ok()) return status;
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
    return current_context_health(*impl_);
}

Result MetalContext::health() const {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh Metal context is not initialized");
    }
    return current_context_health(*impl_);
}

bool MetalContext::is_device_lost() const {
    if (!impl_) return true;
    const fission_skia_status_t status = impl_->health
        ? impl_->health->load()
        : FISSION_SKIA_STATUS_CONTEXT_LOST;
    return status == FISSION_SKIA_STATUS_DEVICE_LOST ||
           status == FISSION_SKIA_STATUS_CONTEXT_LOST ||
           (impl_->ganesh && impl_->ganesh->isDeviceLost());
}

MetalContext::Impl& MetalContext::internal_state() {
    return *impl_;
}

const MetalContext::Impl& MetalContext::internal_state() const {
    return *impl_;
}

}  // namespace fission::skia::ganesh::metal
