//! The presentation comparison: a Godot capture against a 320x224 oracle
//! frame, as the RGB root-mean-square error over the frame or one region.
//!
//! Godot's debug screenshots are normally 1280x800. The field camera renders
//! the retail 320x224 surface at 3x, centred in that viewport, so
//! [`normalize_godot`] crops that surface and samples one pixel from each
//! 3x3 block. An already-normalized 320x224 capture passes through.
//! `tools/certify.py` runs the `presentation-rmse` binary on each pair, and a
//! pair whose recipe names a region is compared over that rectangle only.

use std::fmt;

use psiv_pack::png::{self, Image};

/// The retail frame.
pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 224;
/// Godot's 3x surface.
const SCALE: usize = 3;

/// A capture or frame the comparison can't read, or a region outside the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareError(pub String);

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CompareError {}

impl From<png::PngError> for CompareError {
    fn from(error: png::PngError) -> Self {
        CompareError(error.0)
    }
}

/// An RGB image, three bytes a pixel, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgb {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

/// A rectangle of the 320x224 frame, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Region {
    /// `x,y,width,height`, refused unless it is a non-empty rectangle inside
    /// the frame.
    pub fn parse(text: &str) -> Result<Region, CompareError> {
        let parts = text
            .split(',')
            .map(|part| part.trim().parse::<usize>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| CompareError(format!("region {text:?} is not x,y,width,height")))?;
        let [x, y, width, height] = parts[..] else {
            return Err(CompareError(format!(
                "region {text:?} is not x,y,width,height"
            )));
        };
        let region = Region {
            x,
            y,
            width,
            height,
        };
        if width == 0 || height == 0 || x + width > WIDTH || y + height > HEIGHT {
            return Err(CompareError(format!(
                "region {region} is not inside the {WIDTH}x{HEIGHT} frame"
            )));
        }
        Ok(region)
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{},{}", self.x, self.y, self.width, self.height)
    }
}

/// A decoded PNG as RGB: RGBA and indexed pixels are premultiplied by their
/// alpha onto black, so a transparent pixel is black.
pub fn rgb(image: &Image<'_>) -> Result<Rgb, CompareError> {
    let premultiply = |channel: u8, alpha: u8| (u16::from(channel) * u16::from(alpha) / 255) as u8;
    let mut pixels = Vec::with_capacity(image.width * image.height * 3);
    match image.colour {
        2 => pixels.extend_from_slice(&image.pixels),
        6 => {
            for &[red, green, blue, alpha] in image.pixels.as_chunks::<4>().0 {
                pixels.extend([red, green, blue].map(|c| premultiply(c, alpha)));
            }
        }
        3 => {
            if image.plte.is_empty() {
                return Err(CompareError("indexed PNG has no palette".into()));
            }
            for &index in &image.pixels {
                let start = usize::from(index) * 3;
                let colour = image.plte.get(start..start + 3).ok_or_else(|| {
                    CompareError(format!("pixel index {index} has no palette entry"))
                })?;
                let alpha = image.trns.get(usize::from(index)).copied().unwrap_or(255);
                pixels.extend(colour.iter().map(|&c| premultiply(c, alpha)));
            }
        }
        colour => {
            return Err(CompareError(format!(
                "unsupported PNG colour type {colour}"
            )));
        }
    }
    Ok(Rgb {
        width: image.width,
        height: image.height,
        pixels,
    })
}

/// Read a PNG file's bytes as RGB.
pub fn read_rgb(data: &[u8]) -> Result<Rgb, CompareError> {
    rgb(&png::decode(data)?)
}

/// The capture's retail surface: a 320x224 capture as it is, or one pixel
/// from each 3x3 block of the 960x672 surface centred in a larger one.
pub fn normalize_godot(capture: &Rgb) -> Result<Rgb, CompareError> {
    if (capture.width, capture.height) == (WIDTH, HEIGHT) {
        return Ok(capture.clone());
    }
    let (surface_width, surface_height) = (WIDTH * SCALE, HEIGHT * SCALE);
    if capture.width < surface_width || capture.height < surface_height {
        return Err(CompareError(format!(
            "Godot capture is {}x{}, expected {WIDTH}x{HEIGHT} or at least \
             {surface_width}x{surface_height}",
            capture.width, capture.height
        )));
    }
    let left = (capture.width - surface_width) / 2;
    let top = (capture.height - surface_height) / 2;
    let mut pixels = Vec::with_capacity(WIDTH * HEIGHT * 3);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let source = ((top + y * SCALE) * capture.width + left + x * SCALE) * 3;
            pixels.extend_from_slice(&capture.pixels[source..source + 3]);
        }
    }
    Ok(Rgb {
        width: WIDTH,
        height: HEIGHT,
        pixels,
    })
}

/// The `region` rectangle of a 320x224 frame.
pub fn crop(frame: &Rgb, region: Region) -> Result<Rgb, CompareError> {
    if (frame.width, frame.height) != (WIDTH, HEIGHT) {
        return Err(CompareError(format!(
            "a region is cut from a {WIDTH}x{HEIGHT} frame, not {}x{}",
            frame.width, frame.height
        )));
    }
    let mut pixels = Vec::with_capacity(region.width * region.height * 3);
    for row in region.y..region.y + region.height {
        let start = (row * WIDTH + region.x) * 3;
        pixels.extend_from_slice(&frame.pixels[start..start + region.width * 3]);
    }
    Ok(Rgb {
        width: region.width,
        height: region.height,
        pixels,
    })
}

/// The RGB root-mean-square error over every channel of two equal images.
pub fn rmse(left: &Rgb, right: &Rgb) -> Result<f64, CompareError> {
    if (left.width, left.height) != (right.width, right.height)
        || left.pixels.len() != right.pixels.len()
        || left.pixels.is_empty()
    {
        return Err(CompareError("images do not have equal RGB sizes".into()));
    }
    let squared: u64 = left
        .pixels
        .iter()
        .zip(&right.pixels)
        .map(|(&a, &b)| {
            let difference = u64::from(a.abs_diff(b));
            difference * difference
        })
        .sum();
    Ok((squared as f64 / left.pixels.len() as f64).sqrt())
}

/// One comparison's report line, as `tools/certify.py` reads it:
/// `godot=WxH normalized=320x224 oracle=320x224[ region=x,y,w,h] rmse=R`.
pub fn compare(
    godot_png: &[u8],
    oracle_png: &[u8],
    region: Option<Region>,
) -> Result<String, CompareError> {
    let godot = read_rgb(godot_png)?;
    let oracle = read_rgb(oracle_png)?;
    if (oracle.width, oracle.height) != (WIDTH, HEIGHT) {
        return Err(CompareError(format!(
            "oracle frame is {}x{}, expected {WIDTH}x{HEIGHT}",
            oracle.width, oracle.height
        )));
    }
    let normalized = normalize_godot(&godot)?;
    let (left, right, scope) = match region {
        Some(region) => (
            crop(&normalized, region)?,
            crop(&oracle, region)?,
            format!(" region={region}"),
        ),
        None => (normalized, oracle.clone(), String::new()),
    };
    Ok(format!(
        "godot={}x{} normalized={WIDTH}x{HEIGHT} oracle={}x{}{scope} rmse={:.6}",
        godot.width,
        godot.height,
        oracle.width,
        oracle.height,
        rmse(&left, &right)?
    ))
}

#[cfg(test)]
mod tests;
