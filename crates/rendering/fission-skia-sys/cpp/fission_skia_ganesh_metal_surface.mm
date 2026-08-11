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

#include "include/core/SkColorSpace.h"
#include "include/core/SkImageInfo.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/mtl/SkSurfaceMetal.h"

#import <Cocoa/Cocoa.h>
#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>

#include <climits>
#include <limits>
#include <new>
#include <utility>

// A private subclass lets the runtime reject two live Fission presenters for
// one host view instead of restoring a stale layer when teardown order differs.
@interface FissionSkiaMetalLayer : CAMetalLayer
@end

@implementation FissionSkiaMetalLayer
@end

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

CGFloat backing_scale(NSView* view) {
    NSWindow* window = view.window;
    if (window != nil && window.backingScaleFactor > 0.0) {
        return window.backingScaleFactor;
    }
    NSScreen* screen = window.screen ?: NSScreen.mainScreen;
    return screen != nil && screen.backingScaleFactor > 0.0
        ? screen.backingScaleFactor
        : 1.0;
}

void configure_layer_size(
    CAMetalLayer* layer,
    NSView* view,
    uint32_t width,
    uint32_t height) {
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    layer.frame = view.bounds;
    layer.drawableSize = CGSizeMake(
        static_cast<CGFloat>(width),
        static_cast<CGFloat>(height));
    layer.contentsScale = backing_scale(view);
    [CATransaction commit];
}

Result failed_submission(MetalContext::Impl& context, const char* message) {
    Result health = current_context_health(context);
    if (!health.ok()) return health;
    context.health->record(FISSION_SKIA_STATUS_CONTEXT_LOST);
    return Result::failure(FISSION_SKIA_STATUS_CONTEXT_LOST, message);
}

Result flush_active_surface(
    MetalSurface::Impl& surface,
    SkSurfaces::BackendSurfaceAccess access,
    GrSyncCpu sync) {
    auto& context = surface.context->internal_state();
    Result health = current_context_health(context);
    if (!health.ok()) return health;
    if (!surface.surface) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Metal surface has no active drawable");
    }
    GrFlushInfo flush_info{};
    (void)context.ganesh->flush(surface.surface.get(), access, flush_info);
    if (!context.ganesh->submit(sync)) {
        return failed_submission(
            context,
            "Ganesh rejected the Metal command submission");
    }
    return current_context_health(context);
}

Result lose_surface(MetalSurface::Impl& surface, const char* message) {
    release_active_frame(surface);
    detach_metal_layer(surface, true);
    surface.state = SurfaceState::kLost;
    return Result::failure(FISSION_SKIA_STATUS_SURFACE_LOST, message);
}

}  // namespace

bool attachment_is_current(const MetalSurface::Impl& surface) {
    if (!surface.attachment_installed || !surface.layer ||
        !valid_macos_window(surface.window) || ![NSThread isMainThread]) {
        return false;
    }
    NSView* view = (NSView*)surface.window.ns_view;
    return view.layer == (CAMetalLayer*)surface.layer.get();
}

void release_active_frame(MetalSurface::Impl& surface) {
    surface.surface.reset();
    if (surface.drawable != nullptr) {
        CFRelease(surface.drawable);
        surface.drawable = nullptr;
    }
}

Result attach_metal_layer(MetalSurface::Impl& surface) {
    Result status = validate_host_window(surface.window);
    if (!status.ok()) return status;
    if (surface.width == 0 || surface.height == 0) {
        surface.state = SurfaceState::kSuspended;
        return Result::success();
    }
    auto& context = surface.context->internal_state();
    status = current_context_health(context);
    if (!status.ok()) {
        surface.state = SurfaceState::kLost;
        return status;
    }
    if (!context.device || !context.queue) {
        surface.state = SurfaceState::kLost;
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Metal context has no device or command queue");
    }

    NSView* view = (NSView*)surface.window.ns_view;
    if ([view.layer isKindOfClass:[FissionSkiaMetalLayer class]]) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the NSView already has a live Fission Metal attachment");
    }
    CAMetalLayer* layer = [[FissionSkiaMetalLayer alloc] init];
    if (layer == nil) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the CAMetalLayer could not be allocated");
    }
    surface.layer.reset((GrMTLHandle)layer);
    surface.previous_wants_layer = view.wantsLayer == YES;
    surface.previous_layer.retain((GrMTLHandle)view.layer);

    layer.device = (id<MTLDevice>)context.device.get();
    layer.pixelFormat = MTLPixelFormatBGRA8Unorm_sRGB;
    layer.framebufferOnly = NO;
    layer.opaque = YES;
    layer.presentsWithTransaction = NO;
    layer.contentsGravity = kCAGravityTopLeft;
    layer.magnificationFilter = kCAFilterNearest;
    layer.colorspace = NSColorSpace.sRGBColorSpace.CGColorSpace;
    if (@available(macOS 10.13, *)) {
        layer.displaySyncEnabled = YES;
        layer.maximumDrawableCount = 3;
        layer.allowsNextDrawableTimeout = YES;
    }
    configure_layer_size(layer, view, surface.width, surface.height);

    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    view.layer = layer;
    view.wantsLayer = YES;
    [CATransaction commit];
    if (view.layer != layer) {
        [CATransaction begin];
        [CATransaction setDisableActions:YES];
        view.layer = (CALayer*)surface.previous_layer.get();
        view.wantsLayer = surface.previous_wants_layer ? YES : NO;
        [CATransaction commit];
        surface.layer.reset();
        surface.previous_layer.reset();
        surface.previous_wants_layer = false;
        return Result::failure(
            FISSION_SKIA_STATUS_SURFACE_LOST,
            "AppKit did not install the Fission CAMetalLayer");
    }
    surface.attachment_installed = true;
    surface.state = SurfaceState::kIdle;
    return Result::success();
}

void detach_metal_layer(MetalSurface::Impl& surface, bool drain_queue) {
    release_active_frame(surface);
    if (drain_queue && surface.context != nullptr) {
        auto& context = surface.context->internal_state();
        if (context.health &&
            context.health->load() == FISSION_SKIA_STATUS_OK &&
            context.queue) {
            (void)drain_command_queue(context);
        }
    }

    if (surface.attachment_installed && surface.layer &&
        valid_macos_window(surface.window) && [NSThread isMainThread]) {
        NSView* view = (NSView*)surface.window.ns_view;
        CAMetalLayer* layer = (CAMetalLayer*)surface.layer.get();
        if (view.layer == layer) {
            [CATransaction begin];
            [CATransaction setDisableActions:YES];
            view.layer = (CALayer*)surface.previous_layer.get();
            view.wantsLayer = surface.previous_wants_layer ? YES : NO;
            [CATransaction commit];
        }
    }
    surface.attachment_installed = false;
    surface.layer.reset();
    surface.previous_layer.reset();
    surface.previous_wants_layer = false;
    surface.window = {};
    surface.state = SurfaceState::kSuspended;
}

MetalSurface::MetalSurface(MetalContext& context)
    : impl_(new (std::nothrow) Impl(context)) {}

MetalSurface::~MetalSurface() {
    if (!impl_) return;
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        (void)cancel_frame();
    }
    detach_metal_layer(*impl_, true);
}

Result MetalSurface::create(
    MetalContext& context,
    const MacOSWindow& window,
    uint32_t width,
    uint32_t height,
    std::unique_ptr<MetalSurface>* out_surface) {
    if (out_surface == nullptr || width > INT_MAX || height > INT_MAX) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh Metal surface output or dimensions are invalid");
    }
    out_surface->reset();
    Result status = validate_host_window(window);
    if (!status.ok()) return status;
    status = context.health();
    if (!status.ok()) return status;

    auto surface = std::unique_ptr<MetalSurface>(
        new (std::nothrow) MetalSurface(context));
    if (!surface || !surface->impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the Ganesh Metal surface state could not be allocated");
    }
    surface->impl_->width = width;
    surface->impl_->height = height;
    if (width != 0 && height != 0) {
        surface->impl_->window = window;
        status = attach_metal_layer(*surface->impl_);
        if (!status.ok()) return status;
    }
    *out_surface = std::move(surface);
    return Result::success();
}

Result MetalSurface::resize(
    const MacOSWindow& window,
    uint32_t width,
    uint32_t height) {
    if (!impl_ || width > INT_MAX || height > INT_MAX) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh Metal resize dimensions are invalid");
    }
    Result status = validate_host_window(window);
    if (!status.ok()) return status;
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "a Ganesh Metal surface cannot resize while a frame is active");
    }
    status = impl_->context->health();
    if (!status.ok()) {
        impl_->state = SurfaceState::kLost;
        return status;
    }

    if (width == 0 || height == 0) {
        detach_metal_layer(*impl_, true);
        impl_->width = width;
        impl_->height = height;
        impl_->state = SurfaceState::kSuspended;
        return Result::success();
    }

    const bool same_view = impl_->window.ns_view == window.ns_view;
    if (same_view && attachment_is_current(*impl_)) {
        impl_->width = width;
        impl_->height = height;
        configure_layer_size(
            (CAMetalLayer*)impl_->layer.get(),
            (NSView*)window.ns_view,
            width,
            height);
        impl_->state = SurfaceState::kIdle;
        return Result::success();
    }

    detach_metal_layer(*impl_, true);
    impl_->window = window;
    impl_->width = width;
    impl_->height = height;
    return attach_metal_layer(*impl_);
}

Result MetalSurface::suspend() {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh Metal surface is not initialized");
    }
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "a Ganesh Metal surface cannot suspend while a frame is active");
    }
    detach_metal_layer(*impl_, true);
    impl_->width = 0;
    impl_->height = 0;
    impl_->state = SurfaceState::kSuspended;
    return Result::success();
}

Result MetalSurface::resume(
    const MacOSWindow& window,
    uint32_t width,
    uint32_t height) {
    if (width == 0 || height == 0) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "a resumed Ganesh Metal surface requires a non-zero extent");
    }
    return resize(window, width, height);
}

MetalSurface::Frame MetalSurface::begin_frame() {
    if (!impl_ || impl_->state != SurfaceState::kIdle ||
        !impl_->layer || impl_->width == 0 || impl_->height == 0) {
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_INVALID_STATE,
                "the Ganesh Metal surface is not ready to begin a frame"),
            nullptr,
        };
    }
    if (!attachment_is_current(*impl_)) {
        return Frame{
            lose_surface(
                *impl_,
                "the host replaced or detached the Fission CAMetalLayer"),
            nullptr,
        };
    }
    auto& context = impl_->context->internal_state();
    Result status = current_context_health(context);
    if (!status.ok()) {
        impl_->state = SurfaceState::kLost;
        return Frame{status, nullptr};
    }

    sk_sp<SkSurface> surface;
    GrMTLHandle drawable = nullptr;
    @autoreleasepool {
        surface = SkSurfaces::WrapCAMetalLayer(
            context.ganesh.get(),
            impl_->layer.get(),
            kTopLeft_GrSurfaceOrigin,
            1,
            kBGRA_8888_SkColorType,
            SkColorSpace::MakeSRGB(),
            nullptr,
            &drawable);
    }
    if (!surface || drawable == nullptr ||
        surface->width() != static_cast<int>(impl_->width) ||
        surface->height() != static_cast<int>(impl_->height)) {
        surface.reset();
        if (drawable != nullptr) CFRelease(drawable);
        status = current_context_health(context);
        if (!status.ok()) {
            impl_->state = SurfaceState::kLost;
            return Frame{status, nullptr};
        }
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_SURFACE_LOST,
                "CAMetalLayer did not provide a drawable with the requested extent"),
            nullptr,
        };
    }
    impl_->surface = std::move(surface);
    impl_->drawable = drawable;
    impl_->state = SurfaceState::kRecording;
    SkCanvas* canvas = impl_->surface->getCanvas();
    if (canvas == nullptr) {
        const Result cancelled = cancel_frame();
        return Frame{
            cancelled.ok()
                ? Result::failure(
                    FISSION_SKIA_STATUS_SURFACE_LOST,
                    "the Ganesh Metal drawable has no canvas")
                : cancelled,
            nullptr,
        };
    }
    return Frame{Result::success(), canvas};
}

Result MetalSurface::finish_frame() {
    if (!impl_ || impl_->state != SurfaceState::kRecording) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh Metal surface has no recording frame to finish");
    }
    impl_->state = SurfaceState::kReadyToPresent;
    return Result::success();
}

Result MetalSurface::cancel_frame() {
    if (!impl_ || (impl_->state != SurfaceState::kRecording &&
        impl_->state != SurfaceState::kReadyToPresent)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh Metal surface has no active frame to cancel");
    }
    Result status = flush_active_surface(
        *impl_,
        SkSurfaces::BackendSurfaceAccess::kNoAccess,
        GrSyncCpu::kYes);
    release_active_frame(*impl_);
    impl_->state = status.ok() ? SurfaceState::kIdle : SurfaceState::kLost;
    return status;
}

Result MetalSurface::read_pixels_rgba8888(
    int32_t x,
    int32_t y,
    uint32_t width,
    uint32_t height,
    uint8_t* destination,
    size_t destination_length,
    size_t destination_row_bytes) {
    if (!impl_ || impl_->state != SurfaceState::kReadyToPresent ||
        !impl_->surface) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "Ganesh Metal readback is only valid after frame execution and before present");
    }
    if (x < 0 || y < 0 || width == 0 || height == 0 || destination == nullptr ||
        width > static_cast<uint32_t>(INT_MAX) ||
        height > static_cast<uint32_t>(INT_MAX) ||
        width > std::numeric_limits<size_t>::max() / 4) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh Metal readback request is invalid");
    }
    const size_t minimum_row_bytes = static_cast<size_t>(width) * 4;
    if (destination_row_bytes < minimum_row_bytes ||
        height > std::numeric_limits<size_t>::max() / destination_row_bytes ||
        destination_length < destination_row_bytes * height ||
        static_cast<uint64_t>(x) + width > impl_->width ||
        static_cast<uint64_t>(y) + height > impl_->height) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the Ganesh Metal readback bounds or destination stride are invalid");
    }

    const SkImageInfo info = SkImageInfo::Make(
        static_cast<int>(width),
        static_cast<int>(height),
        kRGBA_8888_SkColorType,
        kPremul_SkAlphaType,
        SkColorSpace::MakeSRGB());
    if (!impl_->surface->readPixels(
            info, destination, destination_row_bytes, x, y)) {
        Result health = impl_->context->health();
        if (!health.ok()) return health;
        return Result::failure(
            FISSION_SKIA_STATUS_SURFACE_LOST,
            "Ganesh could not read the Metal drawable");
    }
    return Result::success();
}

Result MetalSurface::present() {
    if (!impl_ || impl_->state != SurfaceState::kReadyToPresent ||
        !impl_->surface || impl_->drawable == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the Ganesh Metal surface has no completed frame to present");
    }
    if (!attachment_is_current(*impl_)) {
        Result cancelled = cancel_frame();
        detach_metal_layer(*impl_, false);
        impl_->state = SurfaceState::kLost;
        return cancelled.ok()
            ? Result::failure(
                FISSION_SKIA_STATUS_SURFACE_LOST,
                "the host detached the CAMetalLayer before present")
            : cancelled;
    }

    auto& context = impl_->context->internal_state();
    Result status = flush_active_surface(
        *impl_,
        SkSurfaces::BackendSurfaceAccess::kPresent,
        GrSyncCpu::kNo);
    if (!status.ok()) {
        release_active_frame(*impl_);
        impl_->state = SurfaceState::kLost;
        return status;
    }

    @autoreleasepool {
        id<MTLCommandQueue> queue = (id<MTLCommandQueue>)context.queue.get();
        id<CAMetalDrawable> drawable = (id<CAMetalDrawable>)impl_->drawable;
        id<MTLCommandBuffer> command_buffer = [queue commandBuffer];
        if (command_buffer == nil) {
            context.health->record(FISSION_SKIA_STATUS_CONTEXT_LOST);
            release_active_frame(*impl_);
            impl_->state = SurfaceState::kLost;
            return Result::failure(
                FISSION_SKIA_STATUS_CONTEXT_LOST,
                "Metal could not create the presentation command buffer");
        }
        command_buffer.label = @"Fission Ganesh present";
        const std::shared_ptr<DeviceHealth> health = context.health;
        [command_buffer addCompletedHandler:^(id<MTLCommandBuffer> completed) {
            record_command_buffer_completion(health, (GrMTLHandle)completed);
        }];
        [command_buffer presentDrawable:drawable];
        [command_buffer commit];
    }

    release_active_frame(*impl_);
    impl_->state = SurfaceState::kIdle;
    return current_context_health(context);
}

uint32_t MetalSurface::width() const {
    return impl_ ? impl_->width : 0;
}

uint32_t MetalSurface::height() const {
    return impl_ ? impl_->height : 0;
}

bool MetalSurface::is_zero_sized() const {
    return !impl_ || impl_->width == 0 || impl_->height == 0;
}

bool MetalSurface::supports_readback() const {
    return impl_ && impl_->attachment_installed && impl_->layer;
}

}  // namespace fission::skia::ganesh::metal
