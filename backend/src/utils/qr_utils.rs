use reqwest;
use image::DynamicImage;
use bardecoder;
use std::error::Error;

pub async fn scan_qr_code(image_url: &str) -> Result<String, Box<dyn Error>> {
    // Download the image
    let response = reqwest::get(image_url).await?;
    let image_bytes = response.bytes().await?;
    
    // Convert bytes to image
    let img = image::load_from_memory(&image_bytes)?;
    
    // Create QR decoder
    let decoder = bardecoder::default_decoder();
    
    // Decode QR codes
    let results = decoder.decode(&img);
    
    // Return first successful result or empty string
    for result in results {
        if let Ok(data) = result {
            return Ok(data);
        }
    }
    
    Ok(String::new()) // Return empty string if no QR code found
}

