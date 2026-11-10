#[cfg(windows)]
pub fn extract_text_from_jpeg(jpeg_bytes: &[u8]) -> Result<String, String> {
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    if jpeg_bytes.is_empty() {
        return Err("JPEG byte buffer is empty".to_string());
    }

    let stream = InMemoryRandomAccessStream::new().map_err(|e| format!("Failed to create in-memory stream: {e}"))?;
    {
        let output = stream.GetOutputStreamAt(0).map_err(|e| format!("Failed to get output stream: {e}"))?;
        let writer = DataWriter::CreateDataWriter(&output).map_err(|e| format!("Failed to create data writer: {e}"))?;
        writer.WriteBytes(jpeg_bytes).map_err(|e| format!("Failed to write bytes: {e}"))?;
        let _ = writer.StoreAsync().map_err(|e| format!("StoreAsync failed: {e}"))?.get();
        let _ = writer.FlushAsync().map_err(|e| format!("FlushAsync failed: {e}"))?.get();
    }
    stream.Seek(0).map_err(|e| format!("Failed to seek stream to 0: {e}"))?;

    let decoder = BitmapDecoder::CreateAsync(&stream)
        .map_err(|e| format!("Failed to create BitmapDecoder: {e}"))?
        .get()
        .map_err(|e| format!("Decoder get failed: {e}"))?;

    let bitmap = decoder.GetSoftwareBitmapAsync()
        .map_err(|e| format!("Failed to get SoftwareBitmap: {e}"))?
        .get()
        .map_err(|e| format!("SoftwareBitmap get failed: {e}"))?;

    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| format!("Windows OcrEngine creation failed (check installed language packs): {e}"))?;

    let ocr_result = engine.RecognizeAsync(&bitmap)
        .map_err(|e| format!("OCR RecognizeAsync failed: {e}"))?
        .get()
        .map_err(|e| format!("OCR RecognizeAsync get failed: {e}"))?;

    let extracted_text = ocr_result.Text().map_err(|e| format!("Failed to extract OCR text: {e}"))?.to_string();
    Ok(extracted_text)
}

#[cfg(not(windows))]
pub fn extract_text_from_jpeg(_jpeg_bytes: &[u8]) -> Result<String, String> {
    Err("Windows on-device OCR is only supported on Windows operating systems".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_ocr_synthetic_jpeg_on_tokio() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let handle = tokio::spawn(async {
                let mut img = image::RgbaImage::new(200, 100);
                for pixel in img.pixels_mut() {
                    *pixel = image::Rgba([255, 255, 255, 255]);
                }
                let mut jpeg_buf = Cursor::new(Vec::new());
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 85);
                encoder.encode_image(&img).unwrap();
                let jpeg = jpeg_buf.into_inner();
                let ocr = extract_text_from_jpeg(&jpeg);
                println!("Synthetic OCR result ok={:?}, err={:?}", ocr.as_ref().map(|s| s.len()), ocr.as_ref().err());
                assert!(ocr.is_ok(), "OCR failed on Tokio thread: {:?}", ocr.err());
            });
            handle.await.unwrap();
        });
    }
}
