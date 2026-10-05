//! One-shot, lossless desktop capture without a subprocess or temporary file.
use crate::screen::Region;
use image::{
    codecs::png::{CompressionType, FilterType, PngEncoder},
    ImageEncoder,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ, SRCCOPY,
};
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GetWindowDisplayAffinity, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        WINDOW_DISPLAY_AFFINITY,
    },
};

pub struct InkExclusion {
    hwnd: HWND,
    previous: u32,
}
impl InkExclusion {
    pub fn new(hwnd: HWND) -> Result<Self, String> {
        let mut previous = 0;
        unsafe {
            GetWindowDisplayAffinity(hwnd, &mut previous).map_err(|e| e.to_string())?;
            SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE).map_err(|e| e.to_string())?;
        }
        Ok(Self { hwnd, previous })
    }
}
impl Drop for InkExclusion {
    fn drop(&mut self) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.hwnd, WINDOW_DISPLAY_AFFINITY(self.previous));
        }
    }
}

struct CaptureDc {
    screen: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: Option<HGDIOBJ>,
}
impl Drop for CaptureDc {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = self.previous.take() {
                SelectObject(self.memory, previous);
            }
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            }
            if !self.memory.is_invalid() {
                let _ = DeleteDC(self.memory);
            }
            if !self.screen.is_invalid() {
                ReleaseDC(None, self.screen);
            }
        }
    }
}
fn buffer_len(width: u32, height: u32) -> Result<usize, String> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or("Captura demasiado grande")?;
    if width == 0
        || height == 0
        || width > i32::MAX as u32
        || height > i32::MAX as u32
        || pixels > 64 * 1024 * 1024
    {
        return Err("Dimensiones de captura invalidas".into());
    }
    pixels
        .checked_mul(4)
        .ok_or_else(|| "Captura demasiado grande".into())
}
fn opaque_rgba(bytes: &mut [u8]) {
    for pixel in bytes.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
}
#[cfg(test)]
fn crop_png(bytes: &[u8], monitor: Region, region: Region) -> Result<Vec<u8>, String> {
    let x = region.x as i64 - monitor.x as i64;
    let y = region.y as i64 - monitor.y as i64;
    if x < 0 || y < 0 || x + region.width as i64 > monitor.width as i64 || y + region.height as i64 > monitor.height as i64 {
        return Err("Seleccion fuera de la imagen congelada".into());
    }
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?.into_rgba8();
    if image.dimensions() != (monitor.width, monitor.height) { return Err("Dimensiones de seleccion invalidas".into()); }
    let crop = image::imageops::crop_imm(&image, x as u32, y as u32, region.width, region.height).to_image();
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Sub)
        .write_image(crop.as_raw(), region.width, region.height, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())?;
    Ok(png)
}
pub fn rounded_png(bytes: &[u8], scale: f64) -> Result<Vec<u8>, String> {
    let mut image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?.into_rgba8();
    let (width, height) = image.dimensions();
    buffer_len(width, height)?;
    if !scale.is_finite() || scale <= 0. { return Err("Escala de captura invalida".into()); }
    let radius = (14. * scale).min(width as f64 / 2.).min(height as f64 / 2.);
    let extent = radius.ceil() as u32;
    // Antialias only the corner squares; the original interior pixels stay untouched.
    for y in 0..extent {
        for x in 0..extent {
            let mut covered = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let dx = radius - (x as f64 + (sx as f64 + 0.5) / 4.);
                    let dy = radius - (y as f64 + (sy as f64 + 0.5) / 4.);
                    if dx.max(0.).powi(2) + dy.max(0.).powi(2) <= radius * radius { covered += 1; }
                }
            }
            let corners = [(x, y), (width - 1 - x, y), (x, height - 1 - y), (width - 1 - x, height - 1 - y)];
            for (index, &(px, py)) in corners.iter().enumerate() {
                if corners[..index].contains(&(px, py)) { continue; }
                let pixel = image.get_pixel_mut(px, py);
                pixel[3] = ((pixel[3] as u32 * covered + 8) / 16) as u8;
                if pixel[3] == 0 { pixel[0] = 0; pixel[1] = 0; pixel[2] = 0; }
            }
        }
    }
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Sub)
        .write_image(image.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    Ok(png)
}
/// Keep the frozen desktop uncompressed. Only the selected rectangle becomes PNG.
pub struct FrozenFrame {
    region: Region,
    bgra: Vec<u8>,
}
impl FrozenFrame {
    pub fn preview_bmp(&self) -> Vec<u8> {
        // Top-down 32-bit BI_RGB: browsers can display this without PNG compression.
        let mut bmp = Vec::with_capacity(54 + self.bgra.len());
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&((54 + self.bgra.len()) as u32).to_le_bytes());
        bmp.extend_from_slice(&[0; 4]);
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(self.region.width as i32).to_le_bytes());
        bmp.extend_from_slice(&(-(self.region.height as i32)).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&[0; 24]);
        bmp.extend_from_slice(&self.bgra);
        bmp
    }
    pub fn crop_png(&self, region: Region) -> Result<Vec<u8>, String> {
        let x = region.x as i64 - self.region.x as i64;
        let y = region.y as i64 - self.region.y as i64;
        if x < 0 || y < 0 || x + region.width as i64 > self.region.width as i64
            || y + region.height as i64 > self.region.height as i64 {
            return Err("Seleccion fuera de la imagen congelada".into());
        }
        let mut bytes = Vec::with_capacity(buffer_len(region.width, region.height)?);
        let stride = self.region.width as usize * 4;
        for row in y as usize..y as usize + region.height as usize {
            let start = row * stride + x as usize * 4;
            bytes.extend_from_slice(&self.bgra[start..start + region.width as usize * 4]);
        }
        opaque_rgba(&mut bytes);
        encode_png(&bytes, region)
    }
}
pub fn freeze(region: Region) -> Result<FrozenFrame, String> {
    let mut bytes = vec![0; buffer_len(region.width, region.height)?];
    unsafe {
        // Commit pending hide/show changes so repeated captures never include old ink.
        windows::Win32::Graphics::Dwm::DwmFlush().map_err(|e| e.to_string())?;
        let mut dc = CaptureDc {
            screen: GetDC(None),
            memory: HDC::default(),
            bitmap: HBITMAP::default(),
            previous: None,
        };
        if dc.screen.is_invalid() {
            return Err("No se pudo acceder a la pantalla".into());
        }
        dc.memory = CreateCompatibleDC(Some(dc.screen));
        if dc.memory.is_invalid() {
            return Err("No se pudo preparar la captura".into());
        }
        dc.bitmap = CreateCompatibleBitmap(dc.screen, region.width as i32, region.height as i32);
        if dc.bitmap.is_invalid() {
            return Err("No se pudo preparar la imagen".into());
        }
        let previous = SelectObject(dc.memory, HGDIOBJ(dc.bitmap.0));
        if previous.is_invalid() {
            return Err("No se pudo seleccionar la imagen".into());
        }
        dc.previous = Some(previous);
        BitBlt(
            dc.memory,
            0,
            0,
            region.width as i32,
            region.height as i32,
            Some(dc.screen),
            region.x,
            region.y,
            SRCCOPY | CAPTUREBLT,
        )
        .map_err(|e| e.to_string())?;
        // GetDIBits requires the bitmap to be deselected from the memory DC.
        SelectObject(dc.memory, previous);
        dc.previous = None;
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: region.width as i32,
                biHeight: -(region.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        if GetDIBits(
            dc.screen,
            dc.bitmap,
            0,
            region.height,
            Some(bytes.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        ) != region.height as i32
        {
            return Err("No se pudieron leer todos los pixeles".into());
        }
    }
    Ok(FrozenFrame { region, bgra: bytes })
}
pub fn png(region: Region) -> Result<Vec<u8>, String> {
    let mut frame = freeze(region)?;
    opaque_rgba(&mut frame.bgra);
    encode_png(&frame.bgra, region)
}
fn encode_png(bytes: &[u8], region: Region) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Sub)
        .write_image(
            bytes,
            region.width,
            region.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    Ok(png)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_crop_handles_negative_monitor_origin_and_rejects_bounds() {
        let rgba = vec![255u8; 80 * 60 * 4];
        let mut input = Vec::new();
        PngEncoder::new(&mut input).write_image(&rgba, 80, 60, image::ExtendedColorType::Rgba8).unwrap();
        let monitor = Region { x: -80, y: -60, width: 80, height: 60 };
        let crop = crop_png(&input, monitor, Region { x: -60, y: -50, width: 20, height: 15 }).unwrap();
        assert_eq!(image::load_from_memory(&crop).unwrap().into_rgba8().dimensions(), (20, 15));
        assert!(crop_png(&input, monitor, Region { x: -90, y: -50, width: 20, height: 15 }).is_err());
        assert!(crop_png(&input, monitor, Region { x: -10, y: -10, width: 20, height: 15 }).is_err());
    }
    #[test]
    fn raw_frozen_frame_preserves_pixel_order_and_crop_exactly() {
        let region = Region { x: -3, y: -2, width: 3, height: 2 };
        let bgra = vec![1,2,3,0, 4,5,6,0, 7,8,9,0, 10,11,12,0, 13,14,15,0, 16,17,18,0];
        let frame = FrozenFrame { region, bgra };
        let bmp = frame.preview_bmp();
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(bmp.len(), 54 + 24);
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), -2);
        assert_eq!(&bmp[54..58], &[1,2,3,0]);
        let png = frame.crop_png(Region { x: -2, y: -1, width: 2, height: 1 }).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!(decoded.as_raw(), &[15,14,13,255, 18,17,16,255]);
        assert!(frame.crop_png(Region { x: -4, y: -1, width: 2, height: 1 }).is_err());
        assert!(frame.crop_png(Region { x: -2, y: -1, width: 3, height: 1 }).is_err());
        assert!(frame.crop_png(Region { x: -2, y: -1, width: 0, height: 1 }).is_err());
    }
    #[test]
    #[ignore = "Measures native desktop freezing and preview preparation on the local monitor"]
    fn native_selection_preview_latency() {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        let region = Region { x:0, y:0, width:unsafe {GetSystemMetrics(SM_CXSCREEN)} as u32, height:unsafe {GetSystemMetrics(SM_CYSCREEN)} as u32 };
        for _ in 0..5 {
            let started=std::time::Instant::now();
            let frame=freeze(region).unwrap();
            let frozen=started.elapsed();
            let started=std::time::Instant::now();
            let bmp=frame.preview_bmp();
            let preview=started.elapsed();
            let started=std::time::Instant::now();
            let mut rgba=frame.bgra.clone();opaque_rgba(&mut rgba);
            let png=encode_png(&rgba,region).unwrap();
            let old=started.elapsed();
            println!("{}x{} freeze={:?} PNG-preview={:?} BMP-preview={:?} bytes={}/{}",region.width,region.height,frozen,old,preview,png.len(),bmp.len());
        }
    }
    #[test]
    fn rounded_export_has_transparent_antialiased_corners_and_exact_interior() {
        let rgba = vec![101u8; 80 * 60 * 4];
        let mut input = Vec::new();
        PngEncoder::new(&mut input).write_image(&rgba, 80, 60, image::ExtendedColorType::Rgba8).unwrap();
        for scale in [1., 1.5, 2.] {
            let output = rounded_png(&input, scale).unwrap();
            let image = image::load_from_memory(&output).unwrap().into_rgba8();
            assert_eq!(image.dimensions(), (80, 60));
            for (x, y) in [(0, 0), (79, 0), (0, 59), (79, 59)] { assert_eq!(image.get_pixel(x, y)[3], 0); }
            assert_eq!(image.get_pixel(40, 30).0, [101; 4]);
            assert!(image.pixels().any(|p| p[3] > 0 && p[3] < 101));
        }
    }
    #[test]
    fn dimensions_are_checked_before_allocation() {
        assert_eq!(buffer_len(1920, 1080).unwrap(), 1920 * 1080 * 4);
        for (w, h) in [
            (0, 1),
            (1, 0),
            (u32::MAX, 1),
            (1, u32::MAX),
            (100_000, 100_000),
        ] {
            assert!(buffer_len(w, h).is_err());
        }
    }
    #[test]
    fn channels_and_alpha_are_lossless() {
        let mut bytes = [10, 20, 30, 0, 255, 0, 128, 42];
        opaque_rgba(&mut bytes);
        assert_eq!(bytes, [30, 20, 10, 255, 128, 0, 255, 255]);
    }
}
