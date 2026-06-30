# Android native modules

This directory is reserved for native capability module sources copied or owned by the app shell.

Generic dependency and repository wiring is generated into `../native-modules.gradle` from
`fission.toml` `[native]` module declarations. Fission does not ship payment, camera-addon,
scanner-addon, or other app-specific modules in core; those crates provide their native adapters.
