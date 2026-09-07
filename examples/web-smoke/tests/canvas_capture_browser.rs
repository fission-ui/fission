use anyhow::{ensure, Context, Result};
use fission_test_driver::{Bounds, BrowserTestOptions, LiveTestClient};
use serde_json::Value;

const NARROW_SPANISH_COPY: &str =
    "El comité de vecinos revisó con calma el sitio disponible y ordenó los materiales para la jornada común.";

fn changed_pixels_in_bounds(before: &[u8], after: &[u8], bounds: Bounds) -> Result<usize> {
    let before = image::load_from_memory(before)
        .context("decode pre-mutation browser capture")?
        .to_rgba8();
    let after = image::load_from_memory(after)
        .context("decode post-mutation browser capture")?
        .to_rgba8();
    ensure!(before.dimensions() == after.dimensions());

    let left = bounds.x.max(0.0).floor() as u32;
    let top = bounds.y.max(0.0).floor() as u32;
    let right = (bounds.x + bounds.width)
        .ceil()
        .max(0.0)
        .min(before.width() as f32) as u32;
    let bottom = (bounds.y + bounds.height)
        .ceil()
        .max(0.0)
        .min(before.height() as f32) as u32;

    Ok((top..bottom)
        .flat_map(|y| (left..right).map(move |x| (x, y)))
        .filter(|&(x, y)| before.get_pixel(x, y) != after.get_pixel(x, y))
        .count())
}

fn dark_pixels_in_first_line_suffix(png: &[u8], bounds: Bounds) -> Result<usize> {
    let image = image::load_from_memory(png)
        .context("decode Spanish text browser capture")?
        .to_rgba8();
    let left = (bounds.x + bounds.width - 56.0).max(bounds.x).floor() as u32;
    let top = bounds.y.max(0.0).floor() as u32;
    let right = (bounds.x + bounds.width).ceil().min(image.width() as f32) as u32;
    let bottom = (bounds.y + 18.2).ceil().min(image.height() as f32) as u32;

    Ok((top..bottom)
        .flat_map(|y| (left..right).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let pixel = image.get_pixel(x, y).0;
            pixel[3] > 0 && pixel[0] < 96 && pixel[1] < 96 && pixel[2] < 96
        })
        .count())
}

fn browser_url() -> Option<String> {
    std::env::var("FISSION_WEB_SMOKE_URL")
        .ok()
        .map(|url| force_software_renderer(&url))
}

fn force_software_renderer(url: &str) -> String {
    let (page, fragment) = url
        .split_once('#')
        .map_or((url, None), |(page, fragment)| (page, Some(fragment)));
    let (path, query) = page
        .split_once('?')
        .map_or((page, None), |(path, query)| (path, Some(query)));
    let mut parameters = query
        .into_iter()
        .flat_map(|query| query.split('&'))
        .filter(|parameter| {
            !parameter.is_empty()
                && parameter.split_once('=').map_or(*parameter, |(key, _)| key)
                    != "fission_renderer"
        })
        .collect::<Vec<_>>();
    parameters.push("fission_renderer=canvas2d-software");

    let mut forced = format!("{path}?{}", parameters.join("&"));
    if let Some(fragment) = fragment {
        forced.push('#');
        forced.push_str(fragment);
    }
    forced
}

fn launch_software_canvas() -> Result<Option<LiveTestClient>> {
    let Some(url) = browser_url() else {
        eprintln!("set FISSION_WEB_SMOKE_URL to run the canvas capture conformance tests");
        return Ok(None);
    };
    let client = LiveTestClient::launch_browser(BrowserTestOptions::new(url).fission_canvas())?;
    ensure!(
        client
            .browser_report()
            .context("launched browser omitted its renderer report")?
            .renderer
            .as_deref()
            == Some("canvas2d-software"),
        "canvas capture regressions require the software Web presenter"
    );
    Ok(Some(client))
}

#[test]
fn canvas2d_same_size_redraw_does_not_write_dimensions() -> Result<()> {
    let Some(client) = launch_software_canvas()? else {
        return Ok(());
    };
    client.pause_animations()?;
    client.pump()?;
    client.browser_evaluate_json(
        r#"(() => {
            const canvas = document.querySelector('canvas');
            if (!(canvas instanceof HTMLCanvasElement)) {
                throw new Error('Fission canvas is unavailable');
            }
            const prototype = HTMLCanvasElement.prototype;
            const width = Object.getOwnPropertyDescriptor(prototype, 'width');
            const height = Object.getOwnPropertyDescriptor(prototype, 'height');
            if (!width?.get || !width?.set || !height?.get || !height?.set) {
                throw new Error('native canvas dimension descriptors are unavailable');
            }
            const writes = { width: 0, height: 0 };
            globalThis.__FISSION_CANVAS_DIMENSION_WRITES = writes;
            Object.defineProperty(canvas, 'width', {
                configurable: true,
                get() { return width.get.call(this); },
                set(value) {
                    writes.width += 1;
                    width.set.call(this, value);
                },
            });
            Object.defineProperty(canvas, 'height', {
                configurable: true,
                get() { return height.get.call(this); },
                set(value) {
                    writes.height += 1;
                    height.set.call(this, value);
                },
            });
            return true;
        })()"#,
    )?;

    client.pump()?;
    let report = client.browser_evaluate_json(
        r#"(() => {
            const canvas = document.querySelector('canvas');
            const context = canvas?.getContext('2d', { willReadFrequently: true });
            if (!canvas || !context) throw new Error('software canvas is unavailable');
            const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
            const colours = new Set();
            const strideX = Math.max(1, Math.floor(canvas.width / 32));
            const strideY = Math.max(1, Math.floor(canvas.height / 32));
            for (let y = 0; y < canvas.height; y += strideY) {
                for (let x = 0; x < canvas.width; x += strideX) {
                    const offset = (y * canvas.width + x) * 4;
                    colours.add(`${pixels[offset]},${pixels[offset + 1]},${pixels[offset + 2]},${pixels[offset + 3]}`);
                }
            }
            return {
                writes: globalThis.__FISSION_CANVAS_DIMENSION_WRITES,
                colours: colours.size,
            };
        })()"#,
    )?;
    ensure!(report.pointer("/writes/width").and_then(Value::as_u64) == Some(0));
    ensure!(report.pointer("/writes/height").and_then(Value::as_u64) == Some(0));
    ensure!(
        report.get("colours").and_then(Value::as_u64).unwrap_or(0) > 8,
        "stable redraw left the software canvas blank: {report}"
    );
    Ok(())
}

#[test]
fn canvas2d_capture_waits_for_current_composited_frame() -> Result<()> {
    let Some(client) = launch_software_canvas()? else {
        return Ok(());
    };
    client.pause_animations()?;
    client.pump()?;
    let count = client
        .get_text()?
        .into_iter()
        .find(|item| item.text == "Count: 0")
        .context("initial count must be painted")?;
    let spanish = client
        .get_text()?
        .into_iter()
        .find(|item| item.text == NARROW_SPANISH_COPY)
        .context("fixed-width Spanish regression paragraph must be painted")?;
    let spanish_bounds = Bounds {
        x: spanish.x,
        y: spanish.y,
        width: spanish.width,
        height: spanish.height,
    };
    let count_bounds = Bounds {
        x: count.x - 2.0,
        y: count.y - 2.0,
        width: count.width + 4.0,
        height: count.height + 4.0,
    };
    let previous_png = client.capture_screenshot_png()?;
    let before = client
        .browser_evaluate_json("Number(globalThis.__FISSION_RENDERED_FRAME_COUNT || 0)")?
        .as_u64()
        .context("rendered frame count must be an integer")?;
    client.browser_evaluate_json(
        r#"(() => {
            const nativeRequestAnimationFrame = globalThis.requestAnimationFrame.bind(globalThis);
            const state = { frames: [] };
            globalThis.__FISSION_CAPTURE_RAF = state;
            globalThis.requestAnimationFrame = (callback) => nativeRequestAnimationFrame((time) => {
                state.frames.push(Number(globalThis.__FISSION_RENDERED_FRAME_COUNT || 0));
                callback(time);
            });
            return true;
        })()"#,
    )?;

    // Queue a real retained-state mutation without a caller-side pump. The
    // capture operation must pump and present Count: 1 before asking Chrome
    // for the composited viewport; a stale but nonblank Count: 0 frame fails
    // the localized pixel comparison below.
    client.tap_text_without_pump("Increment")?;
    let png = client.capture_screenshot_png()?;
    client.wait_for_text("Count: 1", 2_000)?;
    ensure!(
        changed_pixels_in_bounds(&previous_png, &png, count_bounds)? > 4,
        "capture returned the stale pre-increment count frame"
    );
    ensure!(
        dark_pixels_in_first_line_suffix(&png, spanish_bounds)? > 8,
        "resolved Spanish first-line suffix was blank or clipped in the production browser pipeline"
    );
    let count_bounds_json = serde_json::to_string(&[
        count_bounds.x,
        count_bounds.y,
        count_bounds.width,
        count_bounds.height,
    ])?;
    client.browser_evaluate_json(&format!(
        "globalThis.__FISSION_CAPTURE_COUNT_BOUNDS = {count_bounds_json}; true"
    ))?;
    let report = client.browser_evaluate_json(
        r#"(() => {
            const canvas = document.querySelector('canvas');
            const context = canvas?.getContext('2d', { willReadFrequently: true });
            if (!canvas || !context) throw new Error('software canvas is unavailable');
            const rect = canvas.getBoundingClientRect();
            const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
            const [countX, countY, countWidth, countHeight] =
                globalThis.__FISSION_CAPTURE_COUNT_BOUNDS;
            const cropLeft = Math.max(0, Math.floor(countX));
            const cropTop = Math.max(0, Math.floor(countY));
            const cropRight = Math.min(canvas.width, Math.ceil(countX + countWidth));
            const cropBottom = Math.min(canvas.height, Math.ceil(countY + countHeight));
            const cropWidth = Math.max(0, cropRight - cropLeft);
            const cropHeight = Math.max(0, cropBottom - cropTop);
            const countCrop = context.getImageData(
                cropLeft,
                cropTop,
                cropWidth,
                cropHeight,
            );
            const colours = new Set();
            const strideX = Math.max(1, Math.floor(canvas.width / 32));
            const strideY = Math.max(1, Math.floor(canvas.height / 32));
            for (let y = 0; y < canvas.height; y += strideY) {
                for (let x = 0; x < canvas.width; x += strideX) {
                    const offset = (y * canvas.width + x) * 4;
                    colours.add(`${pixels[offset]},${pixels[offset + 1]},${pixels[offset + 2]},${pixels[offset + 3]}`);
                }
            }
            const samples = [[0.15, 0.15], [0.5, 0.25], [0.25, 0.55], [0.75, 0.7], [0.5, 0.9]]
                .map(([u, v]) => {
                    const x = Math.min(canvas.width - 1, Math.floor(canvas.width * u));
                    const y = Math.min(canvas.height - 1, Math.floor(canvas.height * v));
                    const offset = (y * canvas.width + x) * 4;
                    return {
                        screenX: Math.floor(rect.left + (x + 0.5) * rect.width / canvas.width),
                        screenY: Math.floor(rect.top + (y + 0.5) * rect.height / canvas.height),
                        rgba: Array.from(pixels.slice(offset, offset + 4)),
                    };
                });
            return {
                width: canvas.width,
                height: canvas.height,
                frames: Number(globalThis.__FISSION_RENDERED_FRAME_COUNT || 0),
                rafFrames: globalThis.__FISSION_CAPTURE_RAF?.frames || [],
                colours: colours.size,
                samples,
                countCrop: {
                    left: cropLeft,
                    top: cropTop,
                    width: cropWidth,
                    height: cropHeight,
                    rgba: Array.from(countCrop.data),
                },
            };
        })()"#,
    )?;
    let frames = report
        .get("frames")
        .and_then(Value::as_u64)
        .context("capture report omitted the frame count")?;
    ensure!(
        frames > before,
        "capture did not pump a fresh application frame"
    );
    let callbacks_after_frame = report
        .get("rafFrames")
        .and_then(Value::as_array)
        .context("capture report omitted compositor callbacks")?
        .iter()
        .filter(|frame| frame.as_u64().is_some_and(|frame| frame >= frames))
        .count();
    ensure!(
        callbacks_after_frame >= 2,
        "capture did not cross two compositor frames: {report}"
    );
    ensure!(
        report.get("colours").and_then(Value::as_u64).unwrap_or(0) > 8,
        "software backing store is blank: {report}"
    );

    let image = image::load_from_memory(&png)
        .context("decode composited browser screenshot")?
        .to_rgba8();
    let width = report
        .get("width")
        .and_then(Value::as_u64)
        .context("capture report omitted canvas width")? as u32;
    let height = report
        .get("height")
        .and_then(Value::as_u64)
        .context("capture report omitted canvas height")? as u32;
    ensure!(image.dimensions() == (width, height));

    let count_crop = report
        .get("countCrop")
        .context("capture report omitted the count crop")?;
    let crop_left = count_crop
        .get("left")
        .and_then(Value::as_u64)
        .context("count crop omitted left")? as u32;
    let crop_top = count_crop
        .get("top")
        .and_then(Value::as_u64)
        .context("count crop omitted top")? as u32;
    let crop_width = count_crop
        .get("width")
        .and_then(Value::as_u64)
        .context("count crop omitted width")? as u32;
    let crop_height = count_crop
        .get("height")
        .and_then(Value::as_u64)
        .context("count crop omitted height")? as u32;
    let backing_rgba = count_crop
        .get("rgba")
        .and_then(Value::as_array)
        .context("count crop omitted backing pixels")?;
    ensure!(
        backing_rgba.len() == crop_width as usize * crop_height as usize * 4,
        "count crop byte length disagrees with its dimensions"
    );
    for y in 0..crop_height {
        for x in 0..crop_width {
            let actual = image.get_pixel(crop_left + x, crop_top + y).0;
            let offset = ((y * crop_width + x) * 4) as usize;
            for (channel, expected) in actual.iter().zip(&backing_rgba[offset..offset + 4]) {
                let expected = expected
                    .as_u64()
                    .context("count crop channel is not numeric")?;
                ensure!(
                    (i16::from(*channel) - expected as i16).abs() <= 2,
                    "composited count crop differs from the current software backing store"
                );
            }
        }
    }

    let mut screenshot_colours = std::collections::HashSet::new();
    let stride_x = (width / 32).max(1);
    let stride_y = (height / 32).max(1);
    for y in (0..height).step_by(stride_y as usize) {
        for x in (0..width).step_by(stride_x as usize) {
            screenshot_colours.insert(image.get_pixel(x, y).0);
        }
    }
    ensure!(
        screenshot_colours.len() > 8,
        "composited page screenshot is blank while the backing store is varied"
    );

    for sample in report
        .get("samples")
        .and_then(Value::as_array)
        .context("capture report omitted pixel samples")?
    {
        let x = sample
            .get("screenX")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX) as u32;
        let y = sample
            .get("screenY")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX) as u32;
        ensure!(
            x < width && y < height,
            "sample lies outside capture: {sample}"
        );
        let expected = sample
            .get("rgba")
            .and_then(Value::as_array)
            .context("sample omitted RGBA channels")?;
        let actual = image.get_pixel(x, y).0;
        for (channel, expected) in actual.iter().zip(expected) {
            let expected = expected.as_u64().context("RGBA channel is not numeric")? as i16;
            ensure!(
                (i16::from(*channel) - expected).abs() <= 2,
                "composited pixel differs from backing store at ({x}, {y}): expected={expected:?}, actual={actual:?}"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod url_tests {
    use super::force_software_renderer;

    #[test]
    fn software_renderer_query_precedes_a_fragment() {
        assert_eq!(
            force_software_renderer("http://127.0.0.1:8126/play#scene"),
            "http://127.0.0.1:8126/play?fission_renderer=canvas2d-software#scene"
        );
    }

    #[test]
    fn software_renderer_replaces_an_existing_backend_choice() {
        assert_eq!(
            force_software_renderer(
                "http://127.0.0.1:8126/?fission_renderer=webgpu&mode=test#scene"
            ),
            "http://127.0.0.1:8126/?mode=test&fission_renderer=canvas2d-software#scene"
        );
    }
}
