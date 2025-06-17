
pub fn get_scan_qr_code_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut placeholder_properties = HashMap::new();
    placeholder_properties.insert(
        "param".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("put nothing here".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("scan_qr_code"),
            description: Some(String::from("Scans and extracts data from a QR code in an image. Use this when the user sends an image that appears to contain a QR code.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(placeholder_properties),
                required: None,
            },
        },
    }
}

pub fn get_weather_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut weather_properties = HashMap::new();
    weather_properties.insert(
        "location".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Location of the place where we want to search the weather.".to_string()),
            ..Default::default()
        }),
    );
    weather_properties.insert(
        "units".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Units that the weather should be returned as. Should be either 'metric' or 'imperial'".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("get_weather"),
            description: Some(String::from("Fetches the current weather for the given location. The AI should use the user's home location from user info if none is specified in the query.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(weather_properties),
                required: Some(vec![String::from("location"), String::from("units")]),
            },
        },
    }
}


pub fn get_ask_perplexity_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut plex_properties = HashMap::new();
    plex_properties.insert(
        "query".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The question or topic to get information about".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("ask_perplexity"),
            description: Some(String::from("Get factual or timely information about any topic")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(plex_properties),
                required: Some(vec![String::from("query")]),
            },
        },
    }
}


use reqwest;
use std::error::Error;
use tracing;
use quircs;
use url::Url;

#[derive(Debug)]
pub enum MenuContent {
    Text(String),
    ImageUrl(String),
    PdfUrl(String),
    WebpageUrl(String),
    Unknown(String)
}

pub async fn handle_qr_scan(image_url: Option<&str>) -> String {
    match image_url {
        Some(url) => {
            match scan_qr_code(url).await {
                Ok(menu_content) => {
                    match menu_content {
                        MenuContent::Text(text) => {
                            format!("QR code contains text: {}", text)
                        },
                        MenuContent::ImageUrl(url) => {
                            format!("QR code contains a link to an image: {}", url)
                        },
                        MenuContent::PdfUrl(url) => {
                            format!("QR code contains a link to a PDF: {}", url)
                        },
                        MenuContent::WebpageUrl(url) => {
                            format!("QR code contains a webpage link: {}", url)
                        },
                        MenuContent::Unknown(content) => {
                            format!("QR code content: {}", content)
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Failed to scan QR code: {}", e);
                    "Failed to scan QR code from the image. Please make sure the QR code is clearly visible.".to_string()
                }
            }
        },
        None => {
            "No image was provided in the message. Please send an image containing a QR code.".to_string()
        }
    }
}

pub async fn scan_qr_code(image_url: &str) -> Result<MenuContent, Box<dyn Error>> {
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
                        // Analyze the decoded content
                        if let Ok(url) = Url::parse(&data) {
                            // Check if it's a valid URL
                            let path = url.path().to_lowercase();
                            
                            // Determine content type based on URL
                            if path.ends_with(".pdf") {
                                return Ok(MenuContent::PdfUrl(data));
                            } else if path.ends_with(".jpg") || path.ends_with(".jpeg") 
                                || path.ends_with(".png") || path.ends_with(".webp") {
                                return Ok(MenuContent::ImageUrl(data));
                            } else {
                                // Might be a webpage with menu
                                return Ok(MenuContent::WebpageUrl(data));
                            }
                        } else {
                            // If it's not a URL, return as plain text
                            return Ok(MenuContent::Text(data));
                        }
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
    Ok(MenuContent::Unknown("No QR code found".to_string()))
}

