#include "fission_skia_ganesh_ios_metal_internal.h"

#if !defined(__APPLE__)
#error "The Fission iOS Ganesh Metal runtime is Apple-only"
#endif

#import <TargetConditionals.h>

#if !TARGET_OS_IOS
#error "This module implements the iOS UIKit Metal host"
#endif

#if !defined(FISSION_SKIA_ENABLE_GANESH_IOS_METAL)
#error "Compile this source only when the iOS Ganesh Metal runtime is enabled"
#endif

#include "include/core/SkColorSpace.h"
#include "include/core/SkImageInfo.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/mtl/SkSurfaceMetal.h"

#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>
#import <UIKit/UIKit.h>

#include <climits>
#include <limits>
#include <new>
#include <utility>

@interface FissionSkiaIOSMetalLayer : CAMetalLayer
@end

@implementation FissionSkiaIOSMetalLayer
@end

namespace fission::skia::ganesh::ios_metal {
namespace {

Result validate_host_view(const IOSView& view) {
    if (!valid_ios_view(view)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the iOS native-view descriptor is invalid");
    }
    if (![NSThread isMainThread]) {
        return Result::failure(
            FISSION_SKIA_STATUS_WRONG_THREAD,
            "iOS native-view operations must run on the main thread");
    }
    id object = (id)view.ui_view;
    if (![object isKindOfClass:[UIView class]]) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the iOS native-view descriptor does not contain a UIView");
    }
    return Result::success();
}

bool has_fission_attachment(UIView* view) {
    for (CALayer* candidate in view.layer.sublayers) {
        if ([candidate isKindOfClass:[FissionSkiaIOSMetalLayer class]]) {
            return true;
        }
    }
    return false;
}

CGFloat content_scale(UIView* view) {
    if (view.contentScaleFactor > 0.0) return view.contentScaleFactor;
    UIScreen* screen = view.window.screen ?: UIScreen.mainScreen;
    return screen != nil && screen.scale > 0.0 ? screen.scale : 1.0;
}

void configure_layer_size(
    CAMetalLayer* layer,
    UIView* view,
    uint32_t width,
    uint32_t height) {
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    layer.frame = view.layer.bounds;
    layer.drawableSize = CGSizeMake(
        static_cast<CGFloat>(width),
        static_cast<CGFloat>(height));
    layer.contentsScale = content_scale(view);
    [CATransaction commit];
}

Result failed_submission(IOSMetalContext::Impl& context, const char* message) {
    Result health = current_context_health(context);
    if (!health.ok()) return health;
    context.health->record(FISSION_SKIA_STATUS_CONTEXT_LOST);
    return Result::failure(FISSION_SKIA_STATUS_CONTEXT_LOST, message);
}

Result flush_active_surface(
    IOSMetalSurface::Impl& surface,
    SkSurfaces::BackendSurfaceAccess access,
    GrSyncCpu sync) {
    auto& context = surface.context->internal_state();
    Result health = current_context_health(context);
    if (!health.ok()) return health;
    if (!surface.surface) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the iOS Metal surface has no active drawable");
    }
    GrFlushInfo flush_info{};
    (void)context.ganesh->flush(surface.surface.get(), access, flush_info);
    if (!context.ganesh->submit(sync)) {
        return failed_submission(
            context,
            "Ganesh rejected the iOS Metal command submission");
    }
    return current_context_health(context);
}

Result lose_surface(IOSMetalSurface::Impl& surface, const char* message) {
    release_active_frame(surface);
    detach_metal_layer(surface, true);
    surface.state = SurfaceState::kLost;
    return Result::failure(FISSION_SKIA_STATUS_SURFACE_LOST, message);
}

}  // namespace

bool attachment_is_current(const IOSMetalSurface::Impl& surface) {
    if (!surface.attachment_installed || !surface.layer ||
        !valid_ios_view(surface.view) || ![NSThread isMainThread]) {
        return false;
    }
    UIView* view = (UIView*)surface.view.ui_view;
    return ((CALayer*)surface.layer.get()).superlayer == view.layer;
}

void release_active_frame(IOSMetalSurface::Impl& surface) {
    surface.surface.reset();
    if (surface.drawable != nullptr) {
        CFRelease(surface.drawable);
        surface.drawable = nullptr;
    }
}

Result attach_metal_layer(IOSMetalSurface::Impl& surface) {
    Result status = validate_host_view(surface.view);
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
            "the iOS Metal context has no device or command queue");
    }

    UIView* view = (UIView*)surface.view.ui_view;
    if (has_fission_attachment(view)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the UIView already has a live Fission Metal attachment");
    }
    CAMetalLayer* layer = [[FissionSkiaIOSMetalLayer alloc] init];
    if (layer == nil) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the iOS CAMetalLayer could not be allocated");
    }
    surface.layer.reset((GrMTLHandle)layer);
    layer.device = (id<MTLDevice>)context.device.get();
    layer.pixelFormat = MTLPixelFormatBGRA8Unorm_sRGB;
    layer.framebufferOnly = NO;
    layer.opaque = YES;
    layer.presentsWithTransaction = NO;
    layer.contentsGravity = kCAGravityTopLeft;
    layer.magnificationFilter = kCAFilterNearest;
    CGColorSpaceRef color_space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    if (color_space == nullptr) {
        surface.layer.reset();
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "Core Graphics could not create the iOS sRGB color space");
    }
    layer.colorspace = color_space;
    CGColorSpaceRelease(color_space);
    configure_layer_size(layer, view, surface.width, surface.height);

    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    [view.layer insertSublayer:layer atIndex:0];
    [CATransaction commit];
    if (layer.superlayer != view.layer) {
        [layer removeFromSuperlayer];
        surface.layer.reset();
        return Result::failure(
            FISSION_SKIA_STATUS_SURFACE_LOST,
            "UIKit did not install the Fission CAMetalLayer sublayer");
    }
    surface.attachment_installed = true;
    surface.state = SurfaceState::kIdle;
    return Result::success();
}

void detach_metal_layer(IOSMetalSurface::Impl& surface, bool drain_queue) {
    release_active_frame(surface);
    if (drain_queue && surface.context != nullptr) {
        auto& context = surface.context->internal_state();
        if (context.health &&
            context.health->load() == FISSION_SKIA_STATUS_OK &&
            context.queue) {
            (void)drain_command_queue(context);
        }
    }
    if (surface.layer && [NSThread isMainThread]) {
        [(CALayer*)surface.layer.get() removeFromSuperlayer];
    }
    surface.attachment_installed = false;
    surface.layer.reset();
    surface.view = {};
    surface.state = SurfaceState::kSuspended;
}

IOSMetalSurface::IOSMetalSurface(IOSMetalContext& context)
    : impl_(new (std::nothrow) Impl(context)) {}

IOSMetalSurface::~IOSMetalSurface() {
    if (!impl_) return;
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        (void)cancel_frame();
    }
    detach_metal_layer(*impl_, true);
}

Result IOSMetalSurface::create(
    IOSMetalContext& context,
    const IOSView& view,
    uint32_t width,
    uint32_t height,
    std::unique_ptr<IOSMetalSurface>* out_surface) {
    if (out_surface == nullptr || width > INT_MAX || height > INT_MAX) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the iOS Ganesh Metal surface output or dimensions are invalid");
    }
    out_surface->reset();
    Result status = validate_host_view(view);
    if (!status.ok()) return status;
    status = context.health();
    if (!status.ok()) return status;

    auto surface = std::unique_ptr<IOSMetalSurface>(
        new (std::nothrow) IOSMetalSurface(context));
    if (!surface || !surface->impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_OUT_OF_MEMORY,
            "the iOS Ganesh Metal surface state could not be allocated");
    }
    surface->impl_->width = width;
    surface->impl_->height = height;
    if (width != 0 && height != 0) {
        surface->impl_->view = view;
        status = attach_metal_layer(*surface->impl_);
        if (!status.ok()) return status;
    }
    *out_surface = std::move(surface);
    return Result::success();
}

Result IOSMetalSurface::resize(
    const IOSView& view,
    uint32_t width,
    uint32_t height) {
    if (!impl_ || width > INT_MAX || height > INT_MAX) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the iOS Ganesh Metal resize dimensions are invalid");
    }
    Result status = validate_host_view(view);
    if (!status.ok()) return status;
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "an iOS Ganesh Metal surface cannot resize while a frame is active");
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

    const bool same_view = impl_->view.ui_view == view.ui_view;
    if (same_view && attachment_is_current(*impl_)) {
        impl_->width = width;
        impl_->height = height;
        configure_layer_size(
            (CAMetalLayer*)impl_->layer.get(),
            (UIView*)view.ui_view,
            width,
            height);
        impl_->state = SurfaceState::kIdle;
        return Result::success();
    }

    detach_metal_layer(*impl_, true);
    impl_->view = view;
    impl_->width = width;
    impl_->height = height;
    return attach_metal_layer(*impl_);
}

Result IOSMetalSurface::suspend() {
    if (!impl_) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the iOS Ganesh Metal surface is not initialized");
    }
    if (impl_->state == SurfaceState::kRecording ||
        impl_->state == SurfaceState::kReadyToPresent) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "an iOS Ganesh Metal surface cannot suspend while a frame is active");
    }
    detach_metal_layer(*impl_, true);
    impl_->width = 0;
    impl_->height = 0;
    impl_->state = SurfaceState::kSuspended;
    return Result::success();
}

Result IOSMetalSurface::resume(
    const IOSView& view,
    uint32_t width,
    uint32_t height) {
    if (width == 0 || height == 0) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "a resumed iOS Ganesh Metal surface requires a non-zero extent");
    }
    return resize(view, width, height);
}

IOSMetalSurface::Frame IOSMetalSurface::begin_frame() {
    if (!impl_ || impl_->state != SurfaceState::kIdle ||
        !impl_->layer || impl_->width == 0 || impl_->height == 0) {
        return Frame{
            Result::failure(
                FISSION_SKIA_STATUS_INVALID_STATE,
                "the iOS Ganesh Metal surface is not ready to begin a frame"),
            nullptr,
        };
    }
    if (!attachment_is_current(*impl_)) {
        return Frame{
            lose_surface(
                *impl_,
                "the host detached the Fission CAMetalLayer sublayer"),
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
#if TARGET_OS_SIMULATOR
        if (@available(iOS 13.0, *)) {
            surface = SkSurfaces::WrapCAMetalLayer(
                context.ganesh.get(),
                impl_->layer.get(),
                kTopLeft_GrSurfaceOrigin,
                1,
                kBGRA_8888_SkColorType,
                SkColorSpace::MakeSRGB(),
                nullptr,
                &drawable);
        } else {
            return Frame{
                Result::failure(
                    FISSION_SKIA_STATUS_UNSUPPORTED,
                    "Ganesh CAMetalLayer presentation requires iOS 13 on the simulator"),
                nullptr,
            };
        }
#else
        surface = SkSurfaces::WrapCAMetalLayer(
            context.ganesh.get(),
            impl_->layer.get(),
            kTopLeft_GrSurfaceOrigin,
            1,
            kBGRA_8888_SkColorType,
            SkColorSpace::MakeSRGB(),
            nullptr,
            &drawable);
#endif
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
                "the iOS CAMetalLayer did not provide a drawable with the requested extent"),
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
                    "the iOS Ganesh Metal drawable has no canvas")
                : cancelled,
            nullptr,
        };
    }
    return Frame{Result::success(), canvas};
}

Result IOSMetalSurface::finish_frame() {
    if (!impl_ || impl_->state != SurfaceState::kRecording) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the iOS Ganesh Metal surface has no recording frame to finish");
    }
    impl_->state = SurfaceState::kReadyToPresent;
    return Result::success();
}

Result IOSMetalSurface::cancel_frame() {
    if (!impl_ || (impl_->state != SurfaceState::kRecording &&
        impl_->state != SurfaceState::kReadyToPresent)) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the iOS Ganesh Metal surface has no active frame to cancel");
    }
    Result status = flush_active_surface(
        *impl_,
        SkSurfaces::BackendSurfaceAccess::kNoAccess,
        GrSyncCpu::kYes);
    release_active_frame(*impl_);
    impl_->state = status.ok() ? SurfaceState::kIdle : SurfaceState::kLost;
    return status;
}

Result IOSMetalSurface::read_pixels_rgba8888(
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
            "iOS Ganesh Metal readback is valid only after execution and before present");
    }
    if (x < 0 || y < 0 || width == 0 || height == 0 || destination == nullptr ||
        width > static_cast<uint32_t>(INT_MAX) ||
        height > static_cast<uint32_t>(INT_MAX) ||
        width > std::numeric_limits<size_t>::max() / 4) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the iOS Ganesh Metal readback request is invalid");
    }
    const size_t minimum_row_bytes = static_cast<size_t>(width) * 4;
    if (destination_row_bytes < minimum_row_bytes ||
        height > std::numeric_limits<size_t>::max() / destination_row_bytes ||
        destination_length < destination_row_bytes * height ||
        static_cast<uint64_t>(x) + width > impl_->width ||
        static_cast<uint64_t>(y) + height > impl_->height) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_ARGUMENT,
            "the iOS Ganesh Metal readback bounds or destination stride are invalid");
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
            "Ganesh could not read the iOS Metal drawable");
    }
    return Result::success();
}

Result IOSMetalSurface::present() {
    if (!impl_ || impl_->state != SurfaceState::kReadyToPresent ||
        !impl_->surface || impl_->drawable == nullptr) {
        return Result::failure(
            FISSION_SKIA_STATUS_INVALID_STATE,
            "the iOS Ganesh Metal surface has no completed frame to present");
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
                "Metal could not create the iOS presentation command buffer");
        }
        command_buffer.label = @"Fission iOS Ganesh present";
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

uint32_t IOSMetalSurface::width() const {
    return impl_ ? impl_->width : 0;
}

uint32_t IOSMetalSurface::height() const {
    return impl_ ? impl_->height : 0;
}

bool IOSMetalSurface::is_zero_sized() const {
    return !impl_ || impl_->width == 0 || impl_->height == 0;
}

bool IOSMetalSurface::supports_readback() const {
    return impl_ && impl_->attachment_installed && impl_->layer;
}

}  // namespace fission::skia::ganesh::ios_metal
