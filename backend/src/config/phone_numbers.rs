use std::collections::HashMap;
use once_cell::sync::Lazy;
use std::env;

#[derive(Clone)]
pub struct PhoneConfig {
    pub number: String,
    pub fallback_regions: Vec<String>,  // List of country codes this number can serve as fallback
}

pub static PHONE_NUMBERS: Lazy<HashMap<String, PhoneConfig>> = Lazy::new(|| {
    let mut map = HashMap::new();
    
    // USA configuration
    map.insert("usa".to_string(), PhoneConfig {
        number: env::var("USA_PHONE").expect("No USA phone number set in env"),
        fallback_regions: vec!["can".to_string()], // Canada falls back to USA
    });
    
    // Finland configuration
    map.insert("fin".to_string(), PhoneConfig {
        number: env::var("FIN_PHONE").expect("No FIN phone number set in env"),  
        fallback_regions: vec!["swe".to_string(), "nor".to_string()], // Sweden and Norway fall back to Finland
    });
    
    // Netherlands configuration
    map.insert("nld".to_string(), PhoneConfig {
        number: env::var("NLD_PHONE").expect("No NLD phone number set in env"),
        fallback_regions: vec![
            "deu".to_string(), // Germany
            "bel".to_string(), // Belgium
            "fra".to_string(), // France
            "usa".to_string(), // USA
        ],
    });
    
    map
});


pub fn get_sender_number(user_locality: &str) -> Option<String> {
    // First try direct match
    if let Some(config) = PHONE_NUMBERS.get(user_locality) {
        return Some(config.number.clone());
    }

    // If no direct match, look through fallback regions
    for (_, config) in PHONE_NUMBERS.iter() {
        if config.fallback_regions.contains(&user_locality.to_string()) {
            return Some(config.number.clone());
        }
    }

    // If no match found, return None
    None
}

