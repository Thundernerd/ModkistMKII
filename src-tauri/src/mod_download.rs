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
    let api_key = state.api_key()?;
    let access_token = state.access_token();
    let request_url = if access_token.is_some() {
        download_url.to_string()
    } else {
        with_api_key(download_url, api_key)
    };

    let mut request = reqwest::Client::new()
        .get(request_url)
        .header("X-Modio-Platform", PLATFORM_HEADER);

    if let Some(token) = access_token {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Mod download request failed: {e}"))?;

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
