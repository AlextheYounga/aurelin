use std::time::Duration;

use reqwest::Client;
use sha1::{Digest, Sha1};

const HIBP_RANGE_ENDPOINT: &str = "https://api.pwnedpasswords.com/range";

#[derive(Clone, Debug)]
pub struct CompromisedPasswordChecker {
    client: Client,
    endpoint: String,
}

impl Default for CompromisedPasswordChecker {
    fn default() -> Self {
        Self { client: Client::new(), endpoint: HIBP_RANGE_ENDPOINT.into() }
    }
}

impl CompromisedPasswordChecker {
    pub fn for_endpoint(endpoint: impl Into<String>) -> Self {
        Self { client: Client::new(), endpoint: endpoint.into() }
    }

    pub async fn is_compromised(&self, password: &str) -> Result<bool, reqwest::Error> {
        let digest = Sha1::digest(password.as_bytes());
        let mut hash = String::with_capacity(digest.len() * 2);
        for byte in digest {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            hash.push(char::from(HEX[usize::from(byte >> 4)]));
            hash.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        let (prefix, suffix) = hash.split_at(5);
        let response = self
            .client
            .get(format!("{}/{prefix}", self.endpoint.trim_end_matches('/')))
            .header("Add-Padding", "true")
            .timeout(Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(response.lines().any(|line| {
            let Some((candidate, count)) = line.trim().split_once(':') else {
                return false;
            };
            candidate.eq_ignore_ascii_case(suffix) && count.trim().parse::<u64>().is_ok_and(|count| count > 0)
        }))
    }
}
