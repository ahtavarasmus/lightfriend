use reqwest;
use image::DynamicImage;

use std::error::Error;
use tracing;
use quircs;

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
    
    // Convert to grayscale image
    tracing::info!("Converting to grayscale...");
    let gray_img = img.to_luma8();

    // Create QR decoder
    tracing::info!("Creating QR decoder...");
    let mut decoder = quircs::Quirc::new();

    // Decode QR codes
    tracing::info!("Attempting to decode QR code...");
    let codes = decoder.identify(gray_img.width() as usize, gray_img.height() as usize, &gray_img);

    for (i, code) in codes.enumerate() {
        tracing::info!("Processing code {}", i);
        match code?.decode() {
            Ok(decoded) => {
                match String::from_utf8(decoded.payload) {
                    Ok(data) => {
                        tracing::info!("Successfully decoded QR code: {}", data);
                        return Ok(data);
                    },
                    Err(e) => {
                        tracing::warn!("Failed to convert QR code data to string: {:?}", e);
                    }
                }
            },
            Err(e) => {
                tracing::warn!("Failed to decode code {}: {:?}", i, e);
            }
        }
    }

    tracing::info!("No QR code found in image");
    Ok(String::new()) // Return empty string if no QR code found
}

