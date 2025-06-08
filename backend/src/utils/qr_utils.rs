use reqwest;
use image::DynamicImage;
use bardecoder;
use std::error::Error;
use tracing;

pub async fn scan_qr_code(image_url: &str) -> Result<String, Box<dyn Error>> {
    tracing::info!("Starting QR code scan for URL: {}", image_url);
    
    // Download the image
    tracing::info!("Downloading image...");
    let response = match reqwest::get(image_url).await {
        Ok(resp) => {
            if !resp.status().is_success() {
                tracing::error!("Failed to download image. Status: {}", resp.status());
                return Err(format!("Failed to download image. Status: {}", resp.status()).into());
            }
            resp
        },
        Err(e) => {
            tracing::error!("Failed to make request: {}", e);
            return Err(Box::new(e));
        }
    };
    
    // Get image bytes
    tracing::info!("Getting image bytes...");
    let image_bytes = match response.bytes().await {
        Ok(bytes) => {
            tracing::info!("Downloaded {} bytes", bytes.len());
            bytes
        },
        Err(e) => {
            tracing::error!("Failed to get image bytes: {}", e);
            return Err(Box::new(e));
        }
    };
    
    // Convert bytes to image
    tracing::info!("Converting bytes to image...");
    let img = match image::load_from_memory(&image_bytes) {
        Ok(img) => {
            tracing::info!("Successfully loaded image: {}x{}", img.width(), img.height());
            img
        },
        Err(e) => {
            tracing::error!("Failed to load image from bytes: {}", e);
            return Err(Box::new(e));
        }
    };
    
    // Create QR decoder
    tracing::info!("Creating QR decoder...");
    let decoder = bardecoder::default_decoder();
    
    // Decode QR codes
    tracing::info!("Attempting to decode QR code...");
    let results = decoder.decode(&img);
    
    // Return first successful result or empty string
    for (i, result) in results.into_iter().enumerate() {
        tracing::info!("Processing result {}", i);
        match result {
            Ok(data) => {
                tracing::info!("Successfully decoded QR code: {}", data);
                return Ok(data);
            },
            Err(e) => {
                tracing::warn!("Failed to decode result {}: {:?}", i, e);
            }
        }
    }
    
    tracing::info!("No QR code found in image");
    Ok(String::new()) // Return empty string if no QR code found
}

