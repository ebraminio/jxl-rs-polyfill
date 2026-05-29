use wasm_bindgen::prelude::*;
use jxl::api::*;
use jxl::image::ImageDataType;

#[wasm_bindgen]
pub struct JxlInfo {
    pub width: u32,
    pub height: u32,
    pub num_frames: usize,
    pub has_alpha: bool,
    pub bit_depth: u32,
}

trait JxlPixelType: Copy + Default + ImageDataType + 'static {
    fn data_format() -> JxlDataFormat;
}

impl JxlPixelType for u8 {
    fn data_format() -> JxlDataFormat {
        JxlDataFormat::U8 { bit_depth: 8 }
    }
}

impl JxlPixelType for u16 {
    fn data_format() -> JxlDataFormat {
        JxlDataFormat::U16 { bit_depth: 16, endianness: Endianness::native() }
    }
}

fn decode_jxl_to_rgba<T: JxlPixelType>(data: &[u8]) -> Result<Vec<T>, JsValue> {
    console_error_panic_hook::set_once();

    if data.len() < 2 {
        return Err(JsValue::from_str("Input too small to be a JXL file"));
    }

    let options = JxlDecoderOptions::default();
    let decoder = JxlDecoder::new(options);
    let mut input = data;

    // Advance to image info
    let mut dec = decoder;
    let decoder_with_info = loop {
        match dec.process(&mut input) {
            Ok(ProcessingResult::Complete { result }) => break result,
            Ok(ProcessingResult::NeedsMoreInput { fallback, .. }) => {
                if input.is_empty() {
                    return Err(JsValue::from_str("Incomplete JXL data (header)"));
                }
                dec = fallback;
            }
            Err(e) => return Err(JsValue::from_str(&format!("JXL header error: {}", e))),
        }
    };

    let basic_info = decoder_with_info.basic_info().clone();
    let (width, height) = basic_info.size;

    if width == 0 || height == 0 {
        return Err(JsValue::from_str("Invalid image dimensions"));
    }

    // Build pixel format
    let num_extra_channels = basic_info.extra_channels.len();
    let pixel_format = JxlPixelFormat {
        color_type: JxlColorType::Rgba,
        color_data_format: Some(T::data_format()),
        extra_channel_format: vec![None; num_extra_channels],
    };

    let mut decoder_with_info = decoder_with_info;
    decoder_with_info.set_pixel_format(pixel_format);

    let stride = width * 4;
    let mut current_decoder = decoder_with_info;

    // Advance to frame info
    let decoder_with_frame = loop {
        match current_decoder.process(&mut input) {
            Ok(ProcessingResult::Complete { result }) => break result,
            Ok(ProcessingResult::NeedsMoreInput { fallback, .. }) => {
                if input.is_empty() {
                    return Err(JsValue::from_str("Incomplete JXL data (frame info)"));
                }
                current_decoder = fallback;
            }
            Err(e) => return Err(JsValue::from_str(&format!("JXL frame info error: {}", e))),
        }
    };

    // Allocate flat output buffer and decode directly into it
    let size = stride * height;
    let mut out = vec![T::default(); size];
    {
        let bytes_per_row = stride * std::mem::size_of::<T>();
        // Safety: T is u8 or u16 — primitive types with no padding, valid for any bit pattern.
        let byte_slice = unsafe {
            std::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u8, size * std::mem::size_of::<T>())
        };
        let mut buffers = vec![JxlOutputBuffer::new(byte_slice, height, bytes_per_row)];

        let mut dec3 = decoder_with_frame;
        loop {
            match dec3.process(&mut input, &mut buffers) {
                Ok(ProcessingResult::Complete { .. }) => break,
                Ok(ProcessingResult::NeedsMoreInput { fallback, .. }) => {
                    if input.is_empty() {
                        return Err(JsValue::from_str("Incomplete JXL data (pixels)"));
                    }
                    dec3 = fallback;
                }
                Err(e) => return Err(JsValue::from_str(&format!("JXL decode error: {}", e))),
            }
        }
    }
    Ok(out)
}

/// Decode a JXL image to raw RGBA u8 values
#[wasm_bindgen]
pub fn decode_jxl_to_rgba8(data: &[u8]) -> Result<Vec<u8>, JsValue> {
    decode_jxl_to_rgba::<u8>(data)
}

/// Decode a JXL image to raw RGBA u16 values
#[wasm_bindgen]
pub fn decode_jxl_to_rgba16(data: &[u8]) -> Result<Vec<u16>, JsValue> {
    decode_jxl_to_rgba::<u16>(data)
}

#[wasm_bindgen]
pub fn get_jxl_info(data: &[u8]) -> Result<JxlInfo, JsValue> {
    console_error_panic_hook::set_once();

    if data.len() < 2 {
        return Err(JsValue::from_str("Input too small"));
    }

    let options = JxlDecoderOptions::default();
    let decoder = JxlDecoder::new(options);
    let mut input = data;

    let mut dec = decoder;
    let decoder_with_info = loop {
        match dec.process(&mut input) {
            Ok(ProcessingResult::Complete { result }) => break result,
            Ok(ProcessingResult::NeedsMoreInput { fallback, .. }) => {
                if input.is_empty() {
                    return Err(JsValue::from_str("Incomplete JXL data"));
                }
                dec = fallback;
            }
            Err(e) => return Err(JsValue::from_str(&format!("JXL parse error: {}", e))),
        }
    };

    let info = decoder_with_info.basic_info();
    let has_alpha = !info.extra_channels.is_empty();
    let num_frames = if info.animation.is_some() { 2 } else { 1 }; // Approximate

    Ok(JxlInfo {
        width: info.size.0 as u32,
        height: info.size.1 as u32,
        num_frames,
        has_alpha,
        bit_depth: info.bit_depth.bits_per_sample(),
    })
}
