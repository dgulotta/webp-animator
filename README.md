# webp-animator

A library for converting a series of static WebP images into an animated WebP
image.

Unlike the `webp-animation` crate, this crate is in pure Rust.  This crate does
not know how to encode individual frames, so you will need to use another crate
such as `image` to do that.  The `image` crate is also in pure Rust.

## Example
```rust
use std::fs::File;

use image::{Rgb, RgbImage, codecs::webp::WebPEncoder};
use webp_animator::{Params, WebPAnimator};

fn main() {
    let mut f = File::create("test.webp").unwrap();
    let img1 = RgbImage::from_pixel(64, 64, Rgb([255, 0, 0]));
    let img2 = RgbImage::from_pixel(64, 64, Rgb([0, 0, 255]));
    let params = Params {
        width: 64,
        height: 64,
        background_bgra: [255, 255, 255, 255],
        loop_count: 0,
        has_alpha: false,
    };
    let mut writer = WebPAnimator::new(params).unwrap();
    let mut buf = Vec::new();
    img1.write_with_encoder(WebPEncoder::new_lossless(&mut buf))
        .unwrap();
    writer.add_webp_image(&buf, None, 500).unwrap();
    buf.clear();
    img2.write_with_encoder(WebPEncoder::new_lossless(&mut buf))
        .unwrap();
    writer.add_webp_image(&buf, None, 500).unwrap();
    writer.write(&mut f).unwrap();
}
```

## License
Dual licensed under the [MIT License](LICENSE-MIT) and the
[Apache License, Version 2.0](LICENSE-APACHE).
