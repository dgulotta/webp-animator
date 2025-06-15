//! A library for converting a series of static WebP images into an animated
//! WebP image.
//!
//! Example:
//! ```no_run
//! use std::fs::File;
//! use image::{Rgb, RgbImage, codecs::webp::WebPEncoder};
//! use webp_animator::{Params, WebPAnimator};
//! let mut f = File::create("test.webp").unwrap();
//! let img1 = RgbImage::from_pixel(64, 64, Rgb([255, 0, 0]));
//! let img2 = RgbImage::from_pixel(64, 64, Rgb([0, 0, 255]));
//! let params = Params {
//!     width: 64,
//!     height: 64,
//!     background_bgra: [255, 255, 255, 255],
//!     loop_count: 0,
//!     has_alpha: false,
//! };
//! let mut writer = WebPAnimator::new(params).unwrap();
//! let mut buf = Vec::new();
//! img1.write_with_encoder(WebPEncoder::new_lossless(&mut buf))
//!     .unwrap();
//! writer.add_webp_image(&buf, None, 500).unwrap();
//! buf.clear();
//! img2.write_with_encoder(WebPEncoder::new_lossless(&mut buf))
//!     .unwrap();
//! writer.add_webp_image(&buf, None, 500).unwrap();
//! writer.write(&mut f).unwrap();
//! ```

use std::io::Write;

pub struct WebPAnimator {
    width: u32,
    height: u32,
    icc_profile: Vec<u8>,
    exif_metadata: Vec<u8>,
    xmp_metadata: Vec<u8>,
    frame_data: Vec<u8>,
    background_bgra: [u8; 4],
    loop_count: u16,
    has_alpha: bool,
}

pub struct FrameRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub enum EncodingError {
    InvalidDimensions,
    InvalidDuration,
    UnrecognizedImage,
    Io(std::io::Error),
}

impl core::fmt::Display for EncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimensions => write!(f, "invalid dimensions"),
            Self::InvalidDuration => write!(f, "invalid duration"),
            Self::UnrecognizedImage => write!(f, "unrecognized image"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for EncodingError {}

impl From<std::io::Error> for EncodingError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub width: u32,
    pub height: u32,
    pub background_bgra: [u8; 4],
    pub loop_count: u16,
    pub has_alpha: bool,
}

fn u24_bytes(x: u32) -> [u8; 3] {
    assert!(x >> 24 == 0);
    let b = x.to_le_bytes();
    core::array::from_fn(|i| b[i])
}

impl WebPAnimator {
    pub fn new(params: Params) -> Result<Self, EncodingError> {
        if params.width > 0x1000000 || params.height > 0x1000000 {
            return Err(EncodingError::InvalidDimensions);
        }
        let area = (params.width as u64) * (params.height as u64);
        if area == 0 || (area >> 32) != 0 {
            return Err(EncodingError::InvalidDimensions);
        };
        Ok(Self {
            width: params.width,
            height: params.height,
            icc_profile: Vec::new(),
            exif_metadata: Vec::new(),
            xmp_metadata: Vec::new(),
            frame_data: Vec::new(),
            background_bgra: params.background_bgra,
            loop_count: params.loop_count,
            has_alpha: params.has_alpha,
        })
    }

    pub fn set_icc_profile(&mut self, icc_profile: Vec<u8>) {
        self.icc_profile = icc_profile;
    }

    pub fn set_exif_metadata(&mut self, exif_metadata: Vec<u8>) {
        self.exif_metadata = exif_metadata;
    }

    pub fn set_xmp_metadata(&mut self, xmp_metadata: Vec<u8>) {
        self.xmp_metadata = xmp_metadata;
    }

    /// Add an image to the animation.
    ///
    /// * `data` - A `VP8 ` or `VP8L` chunk.
    /// * `frame` - The frame rectangle.  If `None`, then the frame rectangle
    ///   is assumed to be the entire image.
    /// * `duration` - The duration in milliseconds.
    pub fn add_webp_chunk(
        &mut self,
        data: &[u8],
        frame: Option<FrameRect>,
        duration: u32,
    ) -> Result<(), EncodingError> {
        if !matches!(&data[..4], b"VP8L" | b"VP8 ") {
            return Err(EncodingError::UnrecognizedImage);
        }
        if duration >> 24 != 0 {
            return Err(EncodingError::InvalidDuration);
        }
        let frame = frame.unwrap_or(FrameRect {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        });
        if frame.x & 1 != 0
            || frame.y & 1 != 0
            || frame.x + frame.width > self.width
            || frame.x + frame.height > self.height
        {
            return Err(EncodingError::InvalidDimensions);
        }
        self.frame_data.write_all(b"ANMF")?;
        let chunk_len = data.len() + 16;
        self.frame_data
            .write_all(&(chunk_len as u32).to_le_bytes())?;
        self.frame_data.write_all(&u24_bytes(frame.x >> 1))?;
        self.frame_data.write_all(&u24_bytes(frame.y >> 1))?;
        self.frame_data.write_all(&u24_bytes(frame.width - 1))?;
        self.frame_data.write_all(&u24_bytes(frame.height - 1))?;
        self.frame_data.write_all(&u24_bytes(duration))?;
        self.frame_data.write_all(&[0])?;
        self.frame_data.write_all(data)?;
        Ok(())
    }

    /// Add an image to the animation.
    ///
    /// * `data` - A WebP image.  Currently, only the simple WebP file format
    ///   is supported, meaning that `data` should consist of a header plus
    ///   a single `VP8 ` or `VP8L` chunk.
    /// * `frame` - The frame rectangle.  If `None`, then the frame rectangle
    ///   is assumed to be the entire image.  Frames must have even width and
    ///   height.  In particular, calling this function with `frame=None` will
    ///   fail if the image width or height is odd.
    /// * `duration` - The duration in milliseconds.
    pub fn add_webp_image(
        &mut self,
        data: &[u8],
        frame: Option<FrameRect>,
        duration: u32,
    ) -> Result<(), EncodingError> {
        self.add_webp_chunk(&data[12..], frame, duration)
    }

    const WEBP_HEADER_LEN: usize = 4;
    const VP8X_HEADER_LEN: usize = 18;
    const ANIM_HEADER_LEN: usize = 14;
    const TOTAL_HEADER_LEN: usize =
        Self::WEBP_HEADER_LEN + Self::VP8X_HEADER_LEN + Self::ANIM_HEADER_LEN;

    pub fn write<W: Write + ?Sized>(&mut self, writer: &mut W) -> Result<(), EncodingError> {
        writer.write_all(b"RIFF")?;
        let size = Self::TOTAL_HEADER_LEN
            + self.frame_data.len()
            + self.icc_profile.len()
            + self.exif_metadata.len()
            + self.xmp_metadata.len();
        writer.write_all(&(size as u32).to_le_bytes())?;
        writer.write_all(b"WEBPVP8X")?;
        writer.write_all(&10u32.to_le_bytes())?;
        let icc_flag = if !self.icc_profile.is_empty() {
            0x20
        } else {
            0
        };
        let alpha_flag = if self.has_alpha { 0x10 } else { 0 };
        let exif_flag = if !self.exif_metadata.is_empty() {
            0x8
        } else {
            0
        };
        let xmp_flag = if !self.xmp_metadata.is_empty() {
            0x4
        } else {
            0
        };
        let animation_flag = 0x2;
        let flags = icc_flag | alpha_flag | exif_flag | xmp_flag | animation_flag;
        writer.write_all(&[flags])?;
        writer.write_all(&[0; 3])?;
        writer.write_all(&u24_bytes(self.width - 1))?;
        writer.write_all(&u24_bytes(self.height - 1))?;
        writer.write_all(&self.icc_profile)?;
        writer.write_all(b"ANIM")?;
        writer.write_all(&6u32.to_le_bytes())?;
        writer.write_all(&self.background_bgra)?;
        writer.write_all(&self.loop_count.to_le_bytes())?;
        writer.write_all(&self.frame_data)?;
        writer.write_all(&self.exif_metadata)?;
        writer.write_all(&self.xmp_metadata)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use image::{Rgb, RgbImage, codecs::webp::WebPEncoder};

    use crate::{Params, WebPAnimator};

    #[test]
    fn test_write() {
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
        buf.clear();
        writer.write(&mut buf).unwrap();
        webp_animation::Decoder::new(&buf).unwrap();
    }
}
