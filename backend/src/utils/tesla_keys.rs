use p256::ecdsa::SigningKey;
use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use std::error::Error;
use std::fs;
use std::path::Path;
use tracing::{info, warn};

const PRIVATE_KEY_PATH: &str = "./tesla_private_key.pem";
const PUBLIC_KEY_PATH: &str = "./tesla_public_key.pem";

// Load or generate the EC key pair used to sign Tesla vehicle commands.
//
// Critical invariant: the seeded private key (delivered by entrypoint.sh from
// the host seed server, originally from S3) must NEVER be overwritten. If only
// the private key is present, the public key is derived from it so the keypair
// stays consistent across deploys. Without this, every deploy regenerated both
// halves and silently unpaired every previously-paired vehicle.
pub fn generate_or_load_keys() -> Result<(String, String), Box<dyn Error>> {
    let private_exists = Path::new(PRIVATE_KEY_PATH).exists();
    let public_exists = Path::new(PUBLIC_KEY_PATH).exists();

    if private_exists && public_exists {
        info!("Loading existing Tesla EC key pair");
        let private_key = fs::read_to_string(PRIVATE_KEY_PATH)?;
        let public_key = fs::read_to_string(PUBLIC_KEY_PATH)?;
        return Ok((private_key, public_key));
    }

    if private_exists {
        info!("Deriving Tesla public key from seeded private key");
        let private_pem = fs::read_to_string(PRIVATE_KEY_PATH)?;
        let signing_key = SigningKey::from_pkcs8_pem(&private_pem)
            .map_err(|e| format!("Failed to parse seeded Tesla private key: {}", e))?;
        let public_pem = signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| format!("Failed to derive Tesla public key: {}", e))?;
        fs::write(PUBLIC_KEY_PATH, &public_pem)?;
        info!("Tesla public key derived from seeded private and written to disk");
        return Ok((private_pem, public_pem));
    }

    warn!(
        "No Tesla private key on disk — generating fresh keypair. \
         Any vehicles currently paired with Lightfriend will need to re-pair. \
         To make this keypair persist across deploys, upload {} to \
         s3://$LIGHTFRIEND_BUCKET/config/tesla_private_key.pem after boot.",
        PRIVATE_KEY_PATH
    );

    let signing_key = SigningKey::random(&mut rand::thread_rng());
    let verifying_key = signing_key.verifying_key();

    let private_pem = signing_key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| format!("Failed to encode private key: {}", e))?;

    let public_pem = verifying_key
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| format!("Failed to encode public key: {}", e))?;

    fs::write(PRIVATE_KEY_PATH, &private_pem)?;
    fs::write(PUBLIC_KEY_PATH, &public_pem)?;

    info!("Tesla EC key pair generated and saved");
    info!("Private key: {}", PRIVATE_KEY_PATH);
    info!("Public key: {}", PUBLIC_KEY_PATH);

    Ok((private_pem.to_string(), public_pem))
}

// Get the public key (used for serving via API endpoint)
pub fn get_public_key() -> Result<String, Box<dyn Error>> {
    if Path::new(PUBLIC_KEY_PATH).exists() {
        Ok(fs::read_to_string(PUBLIC_KEY_PATH)?)
    } else {
        let (_, public_key) = generate_or_load_keys()?;
        Ok(public_key)
    }
}
