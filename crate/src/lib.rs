use jxl::api::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct JxlBuffer {
    data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u32,
    pub has_alpha: bool,
    pub is_animated: bool,
}

#[wasm_bindgen]
impl JxlBuffer {
    /// Byte offset of the pixel data within WASM linear memory.
    pub fn ptr(&self) -> u32 {
        self.data.as_ptr() as u32
    }
    /// Total number of bytes in the pixel buffer.
    pub fn byte_len(&self) -> u32 {
        self.data.len() as u32
    }
}

/// Decode a JXL image to raw RGBA pixel data.
/// Automatically selects u8 or u16 per channel based on the image's bit depth.`
#[wasm_bindgen]
pub fn decode_jxl(data: &[u8]) -> Result<JxlBuffer, JsValue> {
    console_error_panic_hook::set_once();

    if data.len() < 2 {
        return Err(JsValue::from_str("Input too small to be a JXL file"));
    }

    let decoder = JxlDecoder::new(JxlDecoderOptions::default());
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
    let bit_depth = basic_info.bit_depth.bits_per_sample();
    let has_alpha = !basic_info.extra_channels.is_empty();
    let is_animated = basic_info.animation.is_some();

    if width == 0 || height == 0 {
        return Err(JsValue::from_str("Invalid image dimensions"));
    }

    let use_u16 = bit_depth > 8;
    let num_extra_channels = basic_info.extra_channels.len();
    let pixel_format = JxlPixelFormat {
        color_type: JxlColorType::Rgba,
        color_data_format: Some(if use_u16 {
            JxlDataFormat::U16 {
                bit_depth: 16,
                endianness: Endianness::native(),
            }
        } else {
            JxlDataFormat::U8 { bit_depth: 8 }
        }),
        extra_channel_format: vec![None; num_extra_channels],
    };

    let mut decoder_with_info = decoder_with_info;
    decoder_with_info.set_pixel_format(pixel_format);

    let bytes_per_sample: usize = if use_u16 { 2 } else { 1 };
    let bytes_per_row = width * 4 * bytes_per_sample;
    let mut pixel_data = vec![0u8; bytes_per_row * height];

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

    {
        let mut buffers = vec![JxlOutputBuffer::new(&mut pixel_data, height, bytes_per_row)];
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

    Ok(JxlBuffer {
        data: pixel_data,
        width: width as u32,
        height: height as u32,
        bit_depth,
        has_alpha,
        is_animated,
    })
}
