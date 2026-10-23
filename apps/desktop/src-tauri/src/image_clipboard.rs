//! Publish a prepared PNG and the Windows bitmap without encoding it again.
#[cfg(windows)]
pub(crate) fn bitmap(image:&image::RgbaImage)->Result<Vec<u8>,String> {
    let (width,height)=image.dimensions();
    if width==0 || height==0 || width>8192 || height>8192 {return Err("Dimensiones de captura invalidas".into());}
    let mut output=vec![0u8;124];
    for (offset,value) in [(0,124),(4,width),(8,height),(16,3),(20,image.as_raw().len() as u32),
        (40,0x00ff0000),(44,0x0000ff00),(48,0x000000ff),(52,0xff000000),(56,0x73524742),(108,4)] {
        output[offset..offset+4].copy_from_slice(&value.to_le_bytes());
    }
    output[12..14].copy_from_slice(&1u16.to_le_bytes());
    output[14..16].copy_from_slice(&32u16.to_le_bytes());
    output.reserve(image.as_raw().len());
    // Positive bottom-up height also works in consumers that reject top-down DIBs.
    // Straight alpha and channel masks match the existing clipboard provider.
    for row in image.as_raw().chunks_exact(width as usize*4).rev() {
        for pixel in row.chunks_exact(4) {output.extend_from_slice(&[pixel[2],pixel[1],pixel[0],pixel[3]]);}
    }
    Ok(output)
}
#[cfg(windows)]
pub(crate) fn write(png:&[u8],dib:&[u8])->Result<(),String> {
    let format=clipboard_win::register_format("PNG").ok_or("No se pudo registrar el formato PNG")?;
    // Preserve the previous provider's six attempts, spaced 5 ms apart, in
    // addition to the export-level retries. Yield-only spins are not equivalent.
    let _clipboard=open()?;
    clipboard_win::raw::empty().map_err(|e|e.to_string())?;
    clipboard_win::raw::set_without_clear(format.get(),png).map_err(|e|e.to_string())?;
    clipboard_win::raw::set_without_clear(clipboard_win::formats::CF_DIBV5,dib).map_err(|e|e.to_string())
}
#[cfg(windows)]
fn open()->Result<clipboard_win::Clipboard,String> {
    for attempt in 0..6 {
        match clipboard_win::Clipboard::new() {
            Ok(clipboard)=>return Ok(clipboard),
            Err(error) if attempt==5=>return Err(error.to_string()),
            Err(_)=>std::thread::sleep(std::time::Duration::from_millis(5)),
        }
    }
    unreachable!()
}
#[cfg(all(test,windows))]
mod tests {
    #[test]
    fn bitmap_keeps_row_order_colors_and_partial_alpha_without_changing_source() {
        let bytes=vec![10,20,30,255,40,50,60,128,70,80,90,0,100,110,120,64];
        let image=image::RgbaImage::from_raw(2,2,bytes.clone()).unwrap();let dib=super::bitmap(&image).unwrap();
        assert_eq!(dib.len(),124+16);assert_eq!(u32::from_le_bytes(dib[8..12].try_into().unwrap()),2);
        assert_eq!(&dib[124..],&[90,80,70,0,120,110,100,64,30,20,10,255,60,50,40,128]);
        assert_eq!(image.as_raw(),&bytes);assert_eq!(u32::from_le_bytes(dib[52..56].try_into().unwrap()),0xff000000);
    }
}
