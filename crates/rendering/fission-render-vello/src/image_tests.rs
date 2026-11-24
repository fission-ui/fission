use super::*;
use std::io::Cursor;
#[cfg(not(target_arch = "wasm32"))]
use std::net::TcpListener;
use std::time::{Duration, Instant};

fn tiny_png() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageOutputFormat::Png)
        .expect("encode png");
    bytes.into_inner()
}

#[cfg(not(target_arch = "wasm32"))]
fn serve_once(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test image server");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0_u8; 1024];
        let _ = std::io::Read::read(&mut stream, &mut request);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        let _ = std::io::Write::write_all(&mut stream, &body);
        let _ = std::io::Write::flush(&mut stream);
    });
    url
}

#[test]
fn oversized_images_are_scaled_to_fit_the_gpu_atlas_while_decoding() {
    let wide = image::RgbaImage::from_pixel(5000, 4, image::Rgba([0, 128, 255, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(wide)
        .write_to(&mut bytes, image::ImageOutputFormat::Png)
        .expect("encode png");

    let pixmap = decode_image_from_bytes(&bytes.into_inner(), None, None).expect("decode wide png");

    assert_eq!(u32::from(pixmap.width()), MAX_DECODED_IMAGE_DIMENSION);
    assert!(
        pixmap.height() >= 1 && pixmap.height() <= 4,
        "aspect ratio kept"
    );
}

#[test]
fn memory_image_load_populates_cache_off_thread() {
    let request = ImageRequest {
        source: ImageSource::Memory {
            bytes: tiny_png(),
            mime_type: Some("image/png".into()),
        },
        cache_width: Some(1),
        cache_height: Some(1),
        ..Default::default()
    };
    let key = request.stable_cache_key();
    IMAGE_CACHE.invalidate(&key);
    IMAGE_CACHE.run_pending_tasks();
    let before = image_cache_generation();

    spawn_image_load(key.clone(), request);

    let deadline = Instant::now() + Duration::from_secs(2);
    while image_cache_generation() == before && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }

    let Some(ImageCacheEntry::Ready(image)) = IMAGE_CACHE.get(&key) else {
        panic!("expected decoded image in cache");
    };
    assert_eq!(image.width(), 1);
    assert_eq!(image.height(), 1);
}

#[test]
fn missing_cache_size_defaults_to_transformed_draw_rect() {
    let request = ImageRequest {
        source: ImageSource::Memory {
            bytes: tiny_png(),
            mime_type: Some("image/png".into()),
        },
        ..Default::default()
    };

    let resolved = image_request_with_default_cache_size(
        &request,
        Rect::new(0.0, 0.0, 80.2, 40.1),
        Affine::scale(2.0),
    );

    assert_eq!(resolved.cache_width, Some(161));
    assert_eq!(resolved.cache_height, Some(81));
}

#[test]
fn explicit_cache_size_is_preserved() {
    let request = ImageRequest {
        source: ImageSource::Memory {
            bytes: tiny_png(),
            mime_type: Some("image/png".into()),
        },
        cache_width: Some(320),
        cache_height: Some(180),
        ..Default::default()
    };

    let resolved = image_request_with_default_cache_size(
        &request,
        Rect::new(0.0, 0.0, 80.0, 40.0),
        Affine::scale(2.0),
    );

    assert_eq!(resolved.cache_width, Some(320));
    assert_eq!(resolved.cache_height, Some(180));
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn network_image_fetch_decodes_png_response() {
    let url = serve_once(tiny_png());
    let image = fetch_network_image(&url, Vec::new(), Some(1), Some(1))
        .expect("fetch and decode test image");

    assert_eq!(image.width(), 1);
    assert_eq!(image.height(), 1);
}
