# Motion Memory Reproducer

This example isolates native renderer memory behaviour for a Cacydil-shaped
screen: a viewport-sized route surface with a tall scroll body.

Run variants with `FISSION_REPRO_SCENARIO`:

```bash
FISSION_REPRO_SCENARIO=plain cargo run -p motion-memory-repro
FISSION_REPRO_SCENARIO=motion cargo run -p motion-memory-repro
FISSION_REPRO_SCENARIO=motion-opacity cargo run -p motion-memory-repro
FISSION_REPRO_SCENARIO=motion-translate cargo run -p motion-memory-repro
FISSION_REPRO_SCENARIO=static-opacity cargo run -p motion-memory-repro
FISSION_REPRO_SCENARIO=plain-images cargo run -p motion-memory-repro
FISSION_REPRO_SCENARIO=motion-images cargo run -p motion-memory-repro
```

Optional shape controls:

```bash
FISSION_REPRO_ROWS=48 FISSION_REPRO_ROW_HEIGHT=96 FISSION_REPRO_IMAGE_PIXELS=1024 cargo run -p motion-memory-repro
FISSION_REPRO_IMAGE_COUNT=1 FISSION_REPRO_SCENARIO=plain-images cargo run -p motion-memory-repro
FISSION_REPRO_CACHE_IMAGES=1 FISSION_REPRO_SCENARIO=motion-images cargo run -p motion-memory-repro
```

On macOS, run the measurement matrix with:

```bash
examples/motion-memory-repro/measure-macos.sh
```

The matrix compares plain rows, visible-only image counts, repeated image cache
keys, unique full-resolution images, explicit cache sizing, and route motion.
