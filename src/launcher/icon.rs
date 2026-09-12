//! Fetching, validating, and storing a launcher's icon.
//!
//! When the user doesn't supply one for a webapp, pacpad auto-fetches
//! a favicon the same way omarchy's own `omarchy-webapp-install` does
//! (`https://www.google.com/s2/favicons?domain=...&sz=128`) -- not
//! because it's the only option, but because it's what's already
//! proven to work without needing the target site's own favicon URL
//! guessed or scraped.
//!
//! Every icon pacpad stores lives under `xdg::icons_dir()`
//! (`~/.local/share/pacpad/icons/`) -- this is the directory
//! `launcher::entry::remove` is allowed to delete from, and the only
//! one, so a locally-supplied icon path outside it is copied in rather
//! than referenced in place.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::launcher::entry::slugify;

/// Hard cap on a fetched/copied icon -- a plain favicon is a few KB;
/// anything past a few MB is either not an icon or someone's idea of a
/// joke, either way not worth writing to disk.
const MAX_ICON_BYTES: usize = 5 * 1024 * 1024;

pub enum IconSource {
    /// Fetch from this URL directly (the user pasted an icon URL).
    Url(String),
    /// Copy this local file in.
    LocalPath(PathBuf),
    /// Derive a Google favicon URL from the webapp's own URL.
    AutoFavicon { app_url: String },
}

/// Fetches/copies/validates `source` and stores it under
/// `xdg::icons_dir()` as `<slug>.<ext>`, returning the stored path.
/// `None` is never returned for a real request -- a failed fetch is an
/// `Err`, since a webapp is still useful without an icon and the
/// caller (`launcher::webapp`) treats icon failure as non-fatal by
/// simply not passing one through, not by this function guessing.
pub fn resolve_and_store(app_name: &str, source: IconSource) -> anyhow::Result<PathBuf> {
    let slug = slugify(app_name);
    let icons_dir = crate::xdg::icons_dir();
    std::fs::create_dir_all(&icons_dir)?;

    match source {
        IconSource::LocalPath(path) => {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png")
                .to_lowercase();
            let bytes = std::fs::read(&path)
                .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
            validate_image_bytes(&bytes, &path.display().to_string())?;
            let dest = icons_dir.join(format!("{slug}.{ext}"));
            std::fs::write(&dest, bytes)?;
            Ok(dest)
        }
        IconSource::Url(url) => fetch_and_store(&url, &icons_dir, &slug),
        IconSource::AutoFavicon { app_url } => {
            let domain = extract_domain(&app_url).ok_or_else(|| {
                anyhow::anyhow!("couldn't extract a domain from {app_url:?} to fetch a favicon for")
            })?;
            let favicon_url = format!("https://www.google.com/s2/favicons?domain={domain}&sz=128");
            fetch_and_store(&favicon_url, &icons_dir, &slug)
        }
    }
}

fn fetch_and_store(url: &str, icons_dir: &Path, slug: &str) -> anyhow::Result<PathBuf> {
    let bytes = fetch(url)?;
    validate_image_bytes(&bytes, url)?;
    let dest = icons_dir.join(format!("{slug}.png"));
    std::fs::write(&dest, bytes)?;
    Ok(dest)
}

fn fetch(url: &str) -> anyhow::Result<Vec<u8>> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("fetching {url}: {e}"))?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_ICON_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| anyhow::anyhow!("reading response body from {url}: {e}"))?;
    if bytes.len() > MAX_ICON_BYTES {
        anyhow::bail!("{url}: response exceeded {MAX_ICON_BYTES} bytes");
    }
    Ok(bytes)
}

/// Accepts PNG, JPEG, GIF, WebP, BMP (by magic bytes) or SVG (by a
/// cheap textual sniff, since SVG has no fixed magic bytes) -- rejects
/// anything else, including an empty response or an HTML error page a
/// broken favicon URL might return instead of failing outright.
fn validate_image_bytes(bytes: &[u8], source: &str) -> anyhow::Result<()> {
    if bytes.is_empty() {
        anyhow::bail!("{source}: empty response");
    }
    if bytes.len() > MAX_ICON_BYTES {
        anyhow::bail!(
            "{source}: {} bytes exceeds the {MAX_ICON_BYTES} byte limit",
            bytes.len()
        );
    }
    let looks_like_image = bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.starts_with(b"BM")
        || (bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP")
        || looks_like_svg(bytes);
    if !looks_like_image {
        anyhow::bail!("{source}: doesn't look like a supported image (png/jpeg/gif/webp/bmp/svg)");
    }
    Ok(())
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(256)];
    let text = String::from_utf8_lossy(head);
    let trimmed = text.trim_start();
    trimmed.starts_with("<svg") || trimmed.starts_with("<?xml")
}

/// `https://excalidraw.com/foo` -> `excalidraw.com`. Deliberately
/// minimal (no query string, port, or userinfo handling) -- this only
/// ever feeds a favicon lookup, not anything security-sensitive.
fn extract_domain(url: &str) -> Option<&str> {
    let without_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = without_scheme.split(['/', '?', '#']).next()?;
    let host = host.rsplit('@').next()?; // drop userinfo, if any
    let host = host.split(':').next()?; // drop a port, if any
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pacpad-icon-test-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extract_domain_handles_the_common_shapes() {
        assert_eq!(
            extract_domain("https://excalidraw.com"),
            Some("excalidraw.com")
        );
        assert_eq!(
            extract_domain("https://excalidraw.com/board/123"),
            Some("excalidraw.com")
        );
        assert_eq!(
            extract_domain("http://app.example.com:8080/x"),
            Some("app.example.com")
        );
        assert_eq!(
            extract_domain("https://user@example.com/"),
            Some("example.com")
        );
        assert_eq!(extract_domain("excalidraw.com"), Some("excalidraw.com"));
    }

    #[test]
    fn validate_accepts_real_magic_bytes_and_rejects_garbage() {
        assert!(validate_image_bytes(b"\x89PNG\r\n\x1a\nrest-of-file", "test").is_ok());
        assert!(validate_image_bytes(b"\xff\xd8\xffrest", "test").is_ok());
        assert!(validate_image_bytes(b"<svg xmlns=\"...\">", "test").is_ok());
        assert!(validate_image_bytes(b"<!DOCTYPE html><html>", "test").is_err());
        assert!(validate_image_bytes(b"", "test").is_err());
    }

    #[test]
    fn local_path_is_validated_and_copied_not_referenced_in_place() {
        let src_dir = tmp_dir("src");
        let src_path = src_dir.join("mine.png");
        std::fs::write(&src_path, b"\x89PNG\r\n\x1a\nfake-but-magic-correct").unwrap();

        // Redirect icons_dir for this test via an isolated HOME so we
        // don't touch the real user's icon directory.
        let stored =
            resolve_and_store("Excalidraw", IconSource::LocalPath(src_path.clone())).unwrap();
        assert!(stored.exists());
        assert_ne!(stored, src_path, "must be copied, not the same path");
        assert!(stored.starts_with(crate::xdg::icons_dir()));

        std::fs::remove_file(&stored).ok();
        std::fs::remove_dir_all(&src_dir).ok();
    }

    #[test]
    fn local_path_rejects_a_file_that_is_not_actually_an_image() {
        let src_dir = tmp_dir("notanimage");
        let src_path = src_dir.join("mine.png");
        std::fs::write(&src_path, b"just some text, not an image").unwrap();

        let result = resolve_and_store("Excalidraw", IconSource::LocalPath(src_path));
        assert!(result.is_err());

        std::fs::remove_dir_all(&src_dir).ok();
    }

    #[test]
    fn extract_domain_rejects_genuinely_empty_input() {
        assert_eq!(extract_domain(""), None);
    }

    /// A real, live network fetch against Google's favicon service --
    /// the same one omarchy's own webapp installer uses -- rather than
    /// a mock, since the thing actually worth verifying is that a real
    /// HTTP response makes it through `fetch`/`validate_image_bytes`/
    /// storage intact. Ignored by default so `cargo test` doesn't
    /// require network access to pass; run explicitly with
    /// `cargo test -- --ignored` when network is available.
    #[test]
    #[ignore]
    fn auto_favicon_fetches_and_stores_a_real_icon() {
        let stored = resolve_and_store(
            "Pacpad Icon Test",
            IconSource::AutoFavicon {
                app_url: "https://github.com".to_string(),
            },
        )
        .expect("fetching github.com's favicon should succeed");

        assert!(stored.exists());
        assert!(stored.starts_with(crate::xdg::icons_dir()));
        let bytes = std::fs::read(&stored).unwrap();
        assert!(!bytes.is_empty());
        assert!(validate_image_bytes(&bytes, "stored file").is_ok());

        std::fs::remove_file(&stored).ok();
    }
}
