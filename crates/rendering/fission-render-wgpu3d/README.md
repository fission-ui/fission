# fission-render-wgpu3d

`fission-render-wgpu3d` is the optional wgpu implementation for Fission's
backend-neutral 3D scene model. It owns wgpu pipelines, shaders, textures, and
device integration so those implementation types do not leak into
`fission-3d-model`.

Most applications enable Fission's `three-d` feature rather than depending on
this crate directly.
