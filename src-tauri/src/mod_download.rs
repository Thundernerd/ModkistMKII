use std::path::Path;

use crate::modio_client::ModioState;

const PLATFORM_HEADER: &str = "windows";

pub fn with_api_key(url: &str, api_key: &str) -> String {
    if url.contains("api_key=") {
        return url.to_string();
    }

    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}api_key={api_key}")
}

pub async fn download_modfile(
    state: &ModioState,
    download_url: &str,
    destination: &Path,
    expected_size: Option<u64>,
) -> Result<(), String> {
    log::info!("Downloading mod file to {}", destination.display());

    if let Some(token) = state.session_token() {
        if let Ok(()) = download_with_bearer(download_url, destination, expected_size, &token).await
        {
            return Ok(());
        }
        log::debug!("OAuth mod download failed, retrying with game API key");
    }

    let api_key = state.api_key()?;
    let request_url = with_api_key(download_url, api_key);
    download_with_api_key(&request_url, destination, expected_size).await
}

async fn download_with_bearer(
    download_url: &str,
    destination: &Path,
    expected_size: Option<u64>,
    token: &str,
) -> Result<(), String> {
    let response = reqwest::Client::new()
        .get(download_url)
        .header("X-Modio-Platform", PLATFORM_HEADER)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Mod download request failed: {e}"))?;

    write_download_response(response, destination, expected_size).await
}

async fn download_with_api_key(
    request_url: &str,
    destination: &Path,
    expected_size: Option<u64>,
) -> Result<(), String> {
    let response = reqwest::Client::new()
        .get(request_url)
        .header("X-Modio-Platform", PLATFORM_HEADER)
        .send()
        .await
        .map_err(|e| format!("Mod download request failed: {e}"))?;

    write_download_response(response, destination, expected_size).await
}

async fn write_download_response(
    response: reqwest::Response,
    destination: &Path,
    expected_size: Option<u64>,
) -> Result<(), String> {
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Mod download failed while reading response: {e}"))?;

    if !status.is_success() {
        let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(300)]);
        return Err(format!(
            "Mod download failed with status {status}. {preview}"
        ));
    }

    if bytes.is_empty() {
        return Err(
            "Downloaded mod file is empty. Sign in to mod.io if this mod requires authentication."
                .into(),
        );
    }

    if let Some(expected) = expected_size {
        if expected > 0 && bytes.len() as u64 != expected {
            return Err(format!(
                "Download incomplete: received {} bytes, expected {expected}. Sign in to mod.io and try again.",
                bytes.len()
            ));
        }
    }

    std::fs::write(destination, &bytes).map_err(|e| {
        format!(
            "Could not write downloaded mod file to {}: {e}",
            destination.display()
        )
    })?;

    log::info!(
        "Downloaded {} bytes to {}",
        bytes.len(),
        destination.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_api_key_query_param() {
        assert_eq!(
            with_api_key("https://g-1.modapi.io/v1/games/1/mods/2/files/3/download", "abc"),
            "https://g-1.modapi.io/v1/games/1/mods/2/files/3/download?api_key=abc"
        );
        assert_eq!(
            with_api_key("https://example.com/file?foo=bar", "abc"),
            "https://example.com/file?foo=bar&api_key=abc"
        );
    }
}
