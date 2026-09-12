//! wgpu device and surface management for the shell.
//!
//! These helpers used to come from classic Vello's `util` module. The sparse strips renderers
//! do not provide them, so the shell owns them now: an instance, the devices created from it,
//! and the window surfaces rendered into through an intermediate target.

use std::fmt;

use wgpu::{
    util::TextureBlitter, Adapter, Device, Instance, Limits, Queue, Surface, SurfaceConfiguration,
    SurfaceTarget, Texture, TextureFormat, TextureView,
};

/// Errors raised while acquiring a device or configuring a surface.
#[derive(Debug)]
pub enum Error {
    /// wgpu could not provide an adapter.
    RequestAdapter(wgpu::RequestAdapterError),
    /// An adapter was found but refused the device request.
    RequestDevice {
        source: wgpu::RequestDeviceError,
        required_features: wgpu::Features,
        required_limits: Box<wgpu::Limits>,
    },
    /// The window surface could not be created.
    CreateSurface(wgpu::CreateSurfaceError),
    /// The surface offers neither `Rgba8Unorm` nor `Bgra8Unorm`.
    UnsupportedSurfaceFormat,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestAdapter(error) => write!(f, "failed to request a wgpu adapter: {error}"),
            Self::RequestDevice {
                source,
                required_features,
                required_limits,
            } => write!(
                f,
                "failed to request a wgpu device (features: {required_features:?}, limits: \
                 {required_limits:?}): {source}"
            ),
            Self::CreateSurface(error) => write!(f, "couldn't create a wgpu surface: {error}"),
            Self::UnsupportedSurfaceFormat => write!(
                f,
                "couldn't find `Rgba8Unorm` or `Bgra8Unorm` texture formats for the surface"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestAdapter(error) => Some(error),
            Self::RequestDevice { source, .. } => Some(source),
            Self::CreateSurface(error) => Some(error),
            Self::UnsupportedSurfaceFormat => None,
        }
    }
}

/// The wgpu instance and every device created from it.
pub struct RenderContext {
    pub instance: Instance,
    pub devices: Vec<DeviceHandle>,
}

/// A device with the adapter and queue it was created from.
pub struct DeviceHandle {
    adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
}

impl DeviceHandle {
    /// The adapter this device was created from.
    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }
}

impl RenderContext {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let backends = wgpu::Backends::from_env().unwrap_or_default();
        let flags = wgpu::InstanceFlags::from_build_config().with_env();
        let memory_budget_thresholds = wgpu::MemoryBudgetThresholds::default();
        let backend_options = wgpu::BackendOptions::from_env_or_default();
        let instance = Instance::new(wgpu::InstanceDescriptor {
            display: None,
            backends,
            flags,
            memory_budget_thresholds,
            backend_options,
        });
        Self {
            instance,
            devices: Vec::new(),
        }
    }

    /// Create a surface for a window and configure it at the given size.
    pub async fn create_surface<'w>(
        &mut self,
        window: impl Into<SurfaceTarget<'w>>,
        width: u32,
        height: u32,
        present_mode: wgpu::PresentMode,
    ) -> Result<RenderSurface<'w>, Error> {
        let surface = self
            .instance
            .create_surface(window.into())
            .map_err(Error::CreateSurface)?;
        self.create_render_surface(surface, width, height, present_mode)
            .await
    }

    /// Configure an existing wgpu surface at the given size.
    pub async fn create_render_surface<'w>(
        &mut self,
        surface: Surface<'w>,
        width: u32,
        height: u32,
        present_mode: wgpu::PresentMode,
    ) -> Result<RenderSurface<'w>, Error> {
        let dev_id = self.device_result(Some(&surface)).await?;

        let device_handle = &self.devices[dev_id];
        let capabilities = surface.get_capabilities(&device_handle.adapter);
        let format = capabilities
            .formats
            .into_iter()
            .find(|it| matches!(it, TextureFormat::Rgba8Unorm | TextureFormat::Bgra8Unorm))
            .ok_or(Error::UnsupportedSurfaceFormat)?;

        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        let (target_texture, target_view) = create_targets(width, height, &device_handle.device);
        let surface = RenderSurface {
            surface,
            config,
            dev_id,
            format,
            target_texture,
            target_view,
            blitter: TextureBlitter::new(&device_handle.device, format),
        };
        self.configure_surface(&surface);
        Ok(surface)
    }

    /// Resize a surface and its intermediate target.
    ///
    /// # Panics
    ///
    /// If `width` or `height` is zero.
    pub fn resize_surface(&self, surface: &mut RenderSurface<'_>, width: u32, height: u32) {
        let (texture, view) = create_targets(width, height, &self.devices[surface.dev_id].device);
        surface.target_texture = texture;
        surface.target_view = view;
        surface.config.width = width;
        surface.config.height = height;
        self.configure_surface(surface);
    }

    pub fn configure_surface(&self, surface: &RenderSurface<'_>) {
        let device = &self.devices[surface.dev_id].device;
        surface.surface.configure(device, &surface.config);
    }

    /// Find or create a compatible device, preserving why creation failed.
    ///
    /// The web shell reports the adapter or device error before falling back to Canvas2D, so
    /// the failure reason must survive rather than collapse to `None`.
    pub async fn device_result(
        &mut self,
        compatible_surface: Option<&Surface<'_>>,
    ) -> Result<usize, Error> {
        let compatible = match compatible_surface {
            Some(surface) => self
                .devices
                .iter()
                .enumerate()
                .find(|(_, handle)| handle.adapter.is_surface_supported(surface))
                .map(|(index, _)| index),
            None => (!self.devices.is_empty()).then_some(0),
        };
        match compatible {
            Some(index) => Ok(index),
            None => self.new_device(compatible_surface).await,
        }
    }

    async fn new_device(
        &mut self,
        compatible_surface: Option<&Surface<'_>>,
    ) -> Result<usize, Error> {
        let adapter =
            wgpu::util::initialize_adapter_from_env_or_default(&self.instance, compatible_surface)
                .await
                .map_err(Error::RequestAdapter)?;
        let features = adapter.features();
        let limits = Limits::default();
        let maybe_features = wgpu::Features::CLEAR_TEXTURE | wgpu::Features::PIPELINE_CACHE;
        let required_features = features & maybe_features;
        log::info!(
            "requesting wgpu device from adapter {:?} with features {:?} and limits {:?}",
            adapter.get_info(),
            required_features,
            limits,
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits: limits.clone(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            })
            .await
            .map_err(|source| Error::RequestDevice {
                source,
                required_features,
                required_limits: Box::new(limits),
            })?;
        self.devices.push(DeviceHandle {
            adapter,
            device,
            queue,
        });
        Ok(self.devices.len() - 1)
    }
}

/// The intermediate texture a frame is rendered into before it is blitted to the surface.
///
/// The compositor samples layer textures and the renderer draws with render passes, so the
/// target is both a render attachment and a sampled texture; the copy usages let the compositor
/// seed and read back layer bases.
fn create_targets(width: u32, height: u32, device: &Device) -> (Texture, TextureView) {
    let target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Fission frame target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        format: TextureFormat::Rgba8Unorm,
        view_formats: &[],
    });
    let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (target_texture, target_view)
}

/// A window surface with its configuration and intermediate target.
pub struct RenderSurface<'s> {
    pub surface: Surface<'s>,
    pub config: SurfaceConfiguration,
    pub dev_id: usize,
    pub format: TextureFormat,
    pub target_texture: Texture,
    pub target_view: TextureView,
    pub blitter: TextureBlitter,
}

impl fmt::Debug for RenderSurface<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderSurface")
            .field("surface", &self.surface)
            .field("config", &self.config)
            .field("dev_id", &self.dev_id)
            .field("format", &self.format)
            .field("target_texture", &self.target_texture)
            .field("target_view", &self.target_view)
            .field("blitter", &"(not Debug)")
            .finish()
    }
}
