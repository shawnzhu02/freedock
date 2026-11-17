//! Snip & ask: Windows' own snipping overlay (ms-screenclip:) puts the snip on the clipboard; we
//! pick it up as a bitmap, shrink it to what Claude reads anyway, and hold it in memory as a PNG
//! until Quick Ask sends it (or it's dropped). Nothing is written to disk.

use base64::Engine;
use std::sync::Mutex;

/// Claude scales images down past this on the long side, so there's no point sending more.
pub const MAX_SIDE: u32 = 1568;

/// The snip waiting to go with the next Quick Ask message, as base64 PNG.
pub static PENDING: Mutex<Option<String>> = Mutex::new(None);

pub fn take() -> Option<String> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).take()
}

pub fn clear() {
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// A packed DIB (BITMAPINFOHEADER + optional masks + pixels, as CF_DIB holds it) to RGBA rows,
/// top-down. 24- and 32-bit, BI_RGB or BI_BITFIELDS; anything else is None.
pub fn dib_to_rgba(dib: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let u32_at = |o: usize| {
        dib.get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let hsize = u32_at(0)? as usize;
    let w = u32_at(4)? as i32;
    let h = u32_at(8)? as i32;
    let bpp = u16::from_le_bytes([*dib.get(14)?, *dib.get(15)?]) as usize;
    let compression = u32_at(16)?;
    let clr_used = u32_at(32).unwrap_or(0) as usize;
    if w <= 0
        || h == 0
        || !(bpp == 24 || bpp == 32)
        || !(compression == 0 || compression == 3)
        || w > 20_000
        || h.abs() > 20_000
    {
        return None;
    }
    // A plain 40-byte header with BI_BITFIELDS is followed by three colour masks.
    let masks = if compression == 3 && hsize == 40 {
        12
    } else {
        0
    };
    let start = hsize + masks + clr_used * 4;
    let (w, top_down, hh) = (w as usize, h < 0, h.unsigned_abs() as usize);
    let stride = (w * bpp).div_ceil(32) * 4;
    let px = dib.get(start..start + stride * hh)?;
    let mut out = vec![0u8; w * hh * 4];
    let mut any_alpha = false;
    for y in 0..hh {
        let src = &px[(if top_down { y } else { hh - 1 - y }) * stride..];
        for x in 0..w {
            let s = &src[x * bpp / 8..];
            let d = &mut out[(y * w + x) * 4..][..4];
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = if bpp == 32 { s[3] } else { 255 };
            any_alpha |= bpp == 32 && s[3] != 0;
        }
    }
    // 32-bit snips usually leave alpha at zero: that means opaque, not invisible.
    if bpp == 32 && !any_alpha {
        out.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    }
    Some((w as u32, hh as u32, out))
}

/// Box-filter downscale so the long side is at most `max`.
pub fn shrink(w: u32, h: u32, rgba: Vec<u8>, max: u32) -> (u32, u32, Vec<u8>) {
    let long = w.max(h);
    if long <= max {
        return (w, h, rgba);
    }
    let (nw, nh) = (
        ((w as u64 * max as u64) / long as u64).max(1) as u32,
        ((h as u64 * max as u64) / long as u64).max(1) as u32,
    );
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        let (y0, y1) = (
            (y * h / nh) as usize,
            (((y + 1) * h / nh).max(y * h / nh + 1)) as usize,
        );
        for x in 0..nw {
            let (x0, x1) = (
                (x * w / nw) as usize,
                (((x + 1) * w / nw).max(x * w / nw + 1)) as usize,
            );
            let mut acc = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let p = &rgba[(sy * w as usize + sx) * 4..][..4];
                    for c in 0..4 {
                        acc[c] += p[c] as u32;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let d = &mut out[((y * nw + x) * 4) as usize..][..4];
            for c in 0..4 {
                d[c] = (acc[c] / n) as u8;
            }
        }
    }
    (nw, nh, out)
}

pub fn png(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = vec![];
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().ok()?;
        wr.write_image_data(rgba).ok()?;
    }
    Some(out)
}

/// A clipboard DIB to base64 PNG, at most MAX_SIDE on the long side.
pub fn from_dib(dib: &[u8]) -> Option<String> {
    let (w, h, px) = dib_to_rgba(dib)?;
    let (w, h, px) = shrink(w, h, px, MAX_SIDE);
    Some(base64::engine::general_purpose::STANDARD.encode(png(w, h, &px)?))
}

#[cfg(windows)]
pub mod win {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
        OpenClipboard,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    const CF_DIB: u32 = 8;

    pub fn sequence() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    pub fn has_image() -> bool {
        unsafe { IsClipboardFormatAvailable(CF_DIB).is_ok() }
    }

    /// The clipboard's CF_DIB bytes, if it holds a bitmap.
    pub fn read_dib() -> Option<Vec<u8>> {
        unsafe {
            OpenClipboard(None).ok()?;
            let out = (|| {
                let h = GetClipboardData(CF_DIB).ok()?;
                let g = HGLOBAL(h.0);
                let n = GlobalSize(g);
                // An 8K screenshot is about 130 MB as a DIB; anything far bigger isn't worth copying.
                if n == 0 || n > 256 * 1024 * 1024 {
                    return None;
                }
                let p = GlobalLock(g) as *const u8;
                if p.is_null() {
                    return None;
                }
                let v = std::slice::from_raw_parts(p, n).to_vec();
                let _ = GlobalUnlock(g);
                Some(v)
            })();
            let _ = CloseClipboard();
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dib(w: i32, h: i32, bpp: u16, rows: &[&[u8]]) -> Vec<u8> {
        let mut v = vec![];
        v.extend(40u32.to_le_bytes());
        v.extend(w.to_le_bytes());
        v.extend(h.to_le_bytes());
        v.extend(1u16.to_le_bytes());
        v.extend(bpp.to_le_bytes());
        v.extend([0u8; 24]);
        for r in rows {
            v.extend(*r);
        }
        v
    }

    #[test]
    fn reads_bottom_up_24_bit_with_padding() {
        // 1x2, rows padded to 4 bytes; bottom row first. Bottom pixel blue, top pixel red (BGR).
        let d = dib(1, 2, 24, &[&[255, 0, 0, 0], &[0, 0, 255, 0]]);
        let (w, h, px) = dib_to_rgba(&d).unwrap();
        assert_eq!((w, h), (1, 2));
        assert_eq!(&px[0..4], &[255, 0, 0, 255], "top row is red");
        assert_eq!(&px[4..8], &[0, 0, 255, 255], "bottom row is blue");
    }

    #[test]
    fn zero_alpha_32_bit_is_opaque() {
        let d = dib(2, 1, 32, &[&[10, 20, 30, 0, 40, 50, 60, 0]]);
        let (_, _, px) = dib_to_rgba(&d).unwrap();
        assert_eq!(px, vec![30, 20, 10, 255, 60, 50, 40, 255]);
    }

    #[test]
    fn rejects_odd_formats_and_short_data() {
        assert!(dib_to_rgba(&dib(1, 1, 8, &[&[0, 0, 0, 0]])).is_none());
        assert!(dib_to_rgba(&dib(4, 4, 32, &[&[0; 8]])).is_none());
        assert!(dib_to_rgba(&[1, 2, 3]).is_none());
    }

    #[test]
    fn shrinks_long_side_and_averages() {
        let (w, h, px) = shrink(
            4,
            2,
            [[0u8, 0, 0, 255], [255, 255, 255, 255]].repeat(4).concat(),
            2,
        );
        assert_eq!((w, h), (2, 1));
        assert_eq!(&px[0..4], &[127, 127, 127, 255]);
        let small = vec![1u8; 16];
        assert_eq!(shrink(2, 2, small.clone(), 10), (2, 2, small));
        assert!(png(1, 1, &[1, 2, 3, 4]).is_some_and(|p| p.starts_with(&[0x89, b'P', b'N', b'G'])));
    }
}
