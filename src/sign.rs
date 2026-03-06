use ethers::utils::hex;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSh256 = Hmac<Sha256>;

pub async fn signature(api_secret: &[u8], msg: &str) -> String {
    let mut mac = HmacSh256::new_from_slice(api_secret).expect("Hmac can take keys of any size..");
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
