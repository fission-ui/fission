# Windows target

Runnable desktop target with release packaging scaffolds for EXE, MSI, and MSIX distribution.

- Run `fission run --project-dir .` from the project root to launch the desktop app and attach output.
- Run `fission build --project-dir . --release` for a release desktop build.
- Run `fission package --target windows --format exe --release --project-dir .` to copy the signed release executable into a package artifact.
- Run `fission package --target windows --format msix --release --project-dir .` or `./platforms/windows/package-msix.ps1` to create an MSIX package with `makeappx`.
- Run `fission package --target windows --format msi --release --project-dir .` or `./platforms/windows/package-msi.ps1` to create an MSI package with WiX.
- Set `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_BASE64`, or `WINDOWS_CERTIFICATE_THUMBPRINT` plus `WINDOWS_CERTIFICATE_PASSWORD` where needed; never commit certificate files or passwords.
- Edit `[package.windows]` in `fission.toml` for Store package identity, publisher identity, package version, and installer preference.
- For an unpackaged NSIS app, build the architecture-matched shortcut helper with `./platforms/windows/build-shortcut-aumid-helper.ps1 -Architecture x64` (or `arm64`) and include `platforms/windows/fission-shortcut-aumid.nsh`. Embed the helper once, then apply one stable AppUserModelID to every Start Menu shortcut after `CreateShortCut`.
- Pass that exact AppUserModelID to `DesktopApp::with_windows_app_user_model_id`; package identity remains authoritative for MSIX, so the explicit value is only the unpackaged fallback.
- Sign the compiled shortcut helper before embedding it in a signed installer. The helper deliberately fails installation if it cannot persist the shortcut identity.
- The generated MSIX manifest stages the desktop executable as a full-trust Windows app and copies `assets/app-icon.png` into the package asset set by default.
