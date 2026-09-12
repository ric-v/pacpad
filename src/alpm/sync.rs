//! Reads a sync repo database: `<DBPath>/sync/<repo>.db`, a compressed
//! tar of `<name>-<version>/desc` entries -- the same `%FIELD%` format
//! as the local db, but with a different field set (`%CSIZE%`/`%ISIZE%`
//! split download vs. installed size; no `%REASON%`/`%INSTALLDATE%`,
//! since sync entries describe what's *available*, not what's
//! installed).
//!
//! The compression is **not uniformly gzip**, despite the `.db`
//! extension and despite every doc/example you'll find online assuming
//! it: upstream Arch's `core.db`/`extra.db` are gzip, but CachyOS's own
//! repos (`cachyos.db`, `cachyos-v3.db`, ...) are Zstandard --
//! confirmed on this machine by magic bytes (`1f 8b` vs. `28 b5 2f
//! fd`), not by any documentation, since pacman's db format has none.
//! `pacman -Sl`/`-Si` read those repos fine, so a reader that only
//! understands gzip silently drops every CachyOS-native repo and
//! misclassifies everything only available there as "foreign" -- this
//! was caught by running against the real system, not by any fixture.
//! So: sniff the magic bytes per file and decompress accordingly,
//! rather than assuming one format for every repo.

use std::io::{Cursor, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use tar::Archive;

use super::desc::DescBlock;

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

/// Decompresses a whole sync db into memory. These files are at most a
/// few megabytes (`extra.db` is the largest on a typical install, ~9
/// MiB compressed) -- decompressing up front and tar-iterating over an
/// in-memory `Cursor` is simpler and just as fast as streaming through
/// two different decoder types, and sidesteps needing a common `Read`
/// trait object for gzip vs. zstd.
fn decompress(bytes: &[u8], db_file: &Path) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    if bytes.starts_with(&ZSTD_MAGIC) {
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(bytes).map_err(|e| {
            anyhow::anyhow!("initializing zstd decoder for {}: {e}", db_file.display())
        })?;
        decoder
            .read_to_end(&mut out)
            .map_err(|e| anyhow::anyhow!("decompressing (zstd) {}: {e}", db_file.display()))?;
    } else if bytes.starts_with(&GZIP_MAGIC) {
        GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .map_err(|e| anyhow::anyhow!("decompressing (gzip) {}: {e}", db_file.display()))?;
    } else {
        anyhow::bail!(
            "{}: unrecognized compression (neither gzip nor zstd magic bytes)",
            db_file.display()
        );
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncPackage {
    pub repo: String,
    pub name: String,
    pub version: String,
    pub base: Option<String>,
    pub description: String,
    pub url: Option<String>,
    pub arch: Option<String>,
    pub license: Vec<String>,
    pub packager: Option<String>,
    pub build_date: Option<i64>,
    pub filename: Option<String>,
    /// `%CSIZE%` -- bytes to download.
    pub download_size: u64,
    /// `%ISIZE%` -- bytes on disk once installed.
    pub installed_size: u64,
    pub depends: Vec<String>,
    pub makedepends: Vec<String>,
    pub optdepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub groups: Vec<String>,
}

impl SyncPackage {
    fn from_desc(repo: &str, d: &DescBlock) -> Option<Self> {
        Some(SyncPackage {
            repo: repo.to_string(),
            name: d.first_owned("NAME")?,
            version: d.first_owned("VERSION")?,
            base: d.first_owned("BASE"),
            description: d.first_owned("DESC").unwrap_or_default(),
            url: d.first_owned("URL"),
            arch: d.first_owned("ARCH"),
            license: d.all_owned("LICENSE"),
            packager: d.first_owned("PACKAGER"),
            build_date: d.first_i64("BUILDDATE"),
            filename: d.first_owned("FILENAME"),
            download_size: d.first_u64("CSIZE").unwrap_or(0),
            installed_size: d.first_u64("ISIZE").unwrap_or(0),
            depends: d.all_owned("DEPENDS"),
            makedepends: d.all_owned("MAKEDEPENDS"),
            optdepends: d.all_owned("OPTDEPENDS"),
            provides: d.all_owned("PROVIDES"),
            conflicts: d.all_owned("CONFLICTS"),
            replaces: d.all_owned("REPLACES"),
            groups: d.all_owned("GROUPS"),
        })
    }
}

/// Parses one repo's `.db` file (a gzip- or zstd-compressed tar of
/// `desc` entries -- see the module docs on why both matter).
///
/// A single unreadable entry inside the tarball is skipped rather than
/// failing the whole repo; a missing or corrupt `.db` file itself is an
/// error, since that's a whole repo silently vanishing from search.
pub fn read_sync_db(db_file: &Path, repo: &str) -> anyhow::Result<Vec<SyncPackage>> {
    let compressed = std::fs::read(db_file)
        .map_err(|e| anyhow::anyhow!("opening sync db {}: {e}", db_file.display()))?;
    let decompressed = decompress(&compressed, db_file)?;

    let mut archive = Archive::new(Cursor::new(decompressed));
    let mut out = Vec::new();

    let entries = archive
        .entries()
        .map_err(|e| anyhow::anyhow!("reading tar entries in {}: {e}", db_file.display()))?;

    // `Archive::entries()` is lazy: for a file that isn't actually
    // gzip/tar at all, the failure doesn't surface here -- it comes
    // back as an `Err` on the *first* iteration instead. So one bad
    // entry among otherwise-good ones is tolerated (skipped), but if
    // every attempt fails and nothing was recovered, that's not an
    // empty repo, it's a file that never parsed at all.
    let mut any_entry_error = false;

    for entry in entries {
        let mut entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                any_entry_error = true;
                continue;
            }
        };
        let is_desc = matches!(entry.path().ok(), Some(p) if p.file_name().and_then(|n| n.to_str()) == Some("desc"));
        if !is_desc {
            continue;
        }
        let mut text = String::new();
        if entry.read_to_string(&mut text).is_err() {
            any_entry_error = true;
            continue;
        }
        if let Some(pkg) = SyncPackage::from_desc(repo, &DescBlock::parse(&text)) {
            out.push(pkg);
        }
    }

    if out.is_empty() && any_entry_error {
        anyhow::bail!(
            "{}: no valid tar entries could be read (corrupt or not gzip/tar)",
            db_file.display()
        );
    }

    Ok(out)
}

/// Reads every repo's `.db` file under `sync_dir`. A repo whose `.db`
/// file is missing (not yet synced) is skipped, not fatal -- the rest
/// of the repos should still be usable.
pub fn read_all_sync_dbs(sync_dir: &Path, repos: &[String]) -> Vec<SyncPackage> {
    let mut out = Vec::new();
    for repo in repos {
        let db_file = sync_dir.join(format!("{repo}.db"));
        if !db_file.exists() {
            continue;
        }
        match read_sync_db(&db_file, repo) {
            Ok(pkgs) => out.extend(pkgs),
            Err(_) => continue,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_db() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sync/extra.db")
    }

    #[test]
    fn reads_all_three_fixture_packages() {
        let pkgs = read_sync_db(&fixture_db(), "extra").unwrap();
        assert_eq!(pkgs.len(), 3);
        let names: Vec<&str> = {
            let mut n: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
            n.sort_unstable();
            n
        };
        assert_eq!(names, vec!["eza", "fd", "ripgrep"]);
    }

    #[test]
    fn repo_tag_and_sizes_are_split_correctly() {
        let pkgs = read_sync_db(&fixture_db(), "extra").unwrap();
        let ripgrep = pkgs.iter().find(|p| p.name == "ripgrep").unwrap();
        assert_eq!(ripgrep.repo, "extra");
        assert_eq!(ripgrep.download_size, 1458382);
        assert_eq!(ripgrep.installed_size, 4494285);
        assert_eq!(
            ripgrep.filename.as_deref(),
            Some("ripgrep-15.2.0-1-x86_64.pkg.tar.zst")
        );
    }

    #[test]
    fn provides_conflicts_replaces_and_makedepends() {
        let pkgs = read_sync_db(&fixture_db(), "extra").unwrap();
        let eza = pkgs.iter().find(|p| p.name == "eza").unwrap();
        assert_eq!(eza.provides, vec!["exa"]);
        assert_eq!(eza.conflicts, vec!["exa"]);
        assert_eq!(eza.replaces, vec!["exa"]);
        assert_eq!(eza.makedepends, vec!["cargo", "pandoc"]);
    }

    #[test]
    fn multiline_license_on_fd() {
        let pkgs = read_sync_db(&fixture_db(), "extra").unwrap();
        let fd = pkgs.iter().find(|p| p.name == "fd").unwrap();
        assert_eq!(fd.license, vec!["MIT", "Apache-2.0"]);
    }

    #[test]
    fn missing_repo_file_is_skipped_by_read_all() {
        let dir = fixture_db().parent().unwrap().to_path_buf();
        let repos = vec!["extra".to_string(), "does-not-exist".to_string()];
        let pkgs = read_all_sync_dbs(&dir, &repos);
        assert_eq!(pkgs.len(), 3); // only extra.db contributed
    }

    #[test]
    fn corrupt_db_file_is_an_error() {
        let bogus = fixture_db().parent().unwrap().join("not-gzip.db");
        std::fs::write(&bogus, b"not actually gzip").unwrap();
        let result = read_sync_db(&bogus, "bogus");
        assert!(result.is_err());
        std::fs::remove_file(&bogus).ok();
    }

    /// Regression test for the real bug caught by running against this
    /// machine: CachyOS's own repos ship `.db` files compressed with
    /// Zstandard, not gzip -- a reader that only understands gzip
    /// silently drops the whole repo and misclassifies everything only
    /// available there as foreign/AUR. `cachyos.db` here is a real
    /// zstd-compressed fixture (built with the `zstd` CLI), not a
    /// gzip file with a renamed extension.
    #[test]
    fn zstd_compressed_sync_db_is_read_correctly() {
        let path = fixture_db().parent().unwrap().join("cachyos.db");
        assert!(
            std::fs::read(&path).unwrap().starts_with(&ZSTD_MAGIC),
            "fixture must actually be zstd-compressed, not just named .db"
        );

        let pkgs = read_sync_db(&path, "cachyos").unwrap();
        assert_eq!(pkgs.len(), 1);
        let pkg = &pkgs[0];
        assert_eq!(pkg.name, "cachyos-alacritty-config");
        assert_eq!(pkg.repo, "cachyos");
        assert_eq!(pkg.version, "1.1-1");
        assert_eq!(pkg.download_size, 4821);
        assert_eq!(pkg.installed_size, 12800);
    }

    #[test]
    fn read_all_sync_dbs_handles_mixed_gzip_and_zstd_repos() {
        let dir = fixture_db().parent().unwrap().to_path_buf();
        let repos = vec!["extra".to_string(), "cachyos".to_string()];
        let pkgs = read_all_sync_dbs(&dir, &repos);
        // 3 from extra.db (gzip) + 1 from cachyos.db (zstd)
        assert_eq!(pkgs.len(), 4);
        assert!(pkgs
            .iter()
            .any(|p| p.name == "ripgrep" && p.repo == "extra"));
        assert!(pkgs
            .iter()
            .any(|p| p.name == "cachyos-alacritty-config" && p.repo == "cachyos"));
    }
}
