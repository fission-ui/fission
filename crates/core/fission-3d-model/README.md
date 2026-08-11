# fission-3d-model

`fission-3d-model` contains Fission-owned, renderer-independent 3D scene data.
It deliberately exposes no wgpu, Skia, window-system, texture, or device type.

Applications normally use the `Scene3D` widget from the `fission` facade. GPU
implementations consume these model types through an optional renderer adapter.
