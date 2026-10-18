use flate2::read::GzDecoder;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};

const MAX_ARCHIVE_BYTES: usize = 100 * 1024 * 1024;
const MAX_SOURCE_BYTES: usize = 50 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_FILES: usize = 5000;

#[derive(Debug, Serialize)]
pub struct SourceSnapshot {
    pub files: BTreeMap<String, String>,
}

fn included(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"]
        .iter()
        .any(|ext| lower.ends_with(ext))
        || lower.ends_with("/tsconfig.json")
        || lower.ends_with("/jsconfig.json")
        || lower == "tsconfig.json"
        || lower == "jsconfig.json"
}

fn parse_archive(bytes: &[u8]) -> Result<SourceSnapshot, String> {
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    let entries = archive
        .entries()
        .map_err(|_| "Could not read the PR-head archive")?;
    for entry in entries {
        let mut entry = entry.map_err(|_| "Could not read the PR-head archive")?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|_| "Invalid archive path")?;
        let parts: Vec<_> = path.components().collect();
        if parts.len() < 2 {
            continue;
        }
        let relative = parts[1..]
            .iter()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if !included(&relative) {
            continue;
        }
        if entry.size() > MAX_FILE_BYTES {
            return Err("A source file exceeds the 50 MiB snapshot limit".into());
        }
        if files.len() >= MAX_FILES {
            return Err("The source snapshot has too many files".into());
        }
        total = total
            .checked_add(entry.size() as usize)
            .ok_or("Source snapshot is too large")?;
        if total > MAX_SOURCE_BYTES {
            return Err("The source snapshot exceeds 50 MiB".into());
        }
        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|_| "A source file is not UTF-8 text")?;
        if files.insert(relative, content).is_some() {
            return Err("The source snapshot has duplicate paths".into());
        }
    }
    if files.is_empty() {
        return Err("No TS/JS source was found in the PR-head archive".into());
    }
    Ok(SourceSnapshot { files })
}

pub async fn load(
    owner: &str,
    repo: &str,
    head_sha: &str,
    token: &str,
) -> Result<SourceSnapshot, String> {
    if ![owner, repo].iter().all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    }) || head_sha.len() != 40
        || !head_sha.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Invalid repository or PR-head SHA".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|_| "Could not create GitHub client")?;
    let url = format!("https://api.github.com/repos/{owner}/{repo}/tarball/{head_sha}");
    let mut response = client
        .get(url)
        .bearer_auth(token)
        .header("User-Agent", "Revisor")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| "Could not download the PR-head archive")?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub archive HTTP {}",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Could not download the PR-head archive")?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_ARCHIVE_BYTES {
            return Err("The compressed PR-head archive exceeds 100 MiB".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    tokio::task::spawn_blocking(move || parse_archive(&bytes))
        .await
        .map_err(|_| String::from("Could not read the PR-head archive"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;
    #[test]
    fn keeps_source_and_config_from_archive() {
        let mut encoded = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut tar = tar::Builder::new(&mut encoded);
            for (path, body) in [
                ("root/src/a.ts", "export function a() {}"),
                ("root/tsconfig.json", "{}"),
                ("root/README.md", "ignore"),
            ] {
                let mut header = tar::Header::new_gnu();
                header.set_size(body.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                tar.append_data(&mut header, path, body.as_bytes()).unwrap();
            }
            tar.finish().unwrap();
        }
        encoded.flush().unwrap();
        let files = parse_archive(&encoded.finish().unwrap()).unwrap().files;
        assert_eq!(files.len(), 2);
        assert!(files.contains_key("src/a.ts"));
        assert!(files.contains_key("tsconfig.json"));
    }
    fn archive_with_source_size(size: usize) -> Vec<u8> {
        let mut encoded = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut tar = tar::Builder::new(&mut encoded);
            let body = vec![b'a'; size];
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, "root/src/large.ts", body.as_slice())
                .unwrap();
            tar.finish().unwrap();
        }
        encoded.finish().unwrap()
    }

    #[test]
    fn accepts_source_file_above_old_two_mib_limit() {
        let bytes = archive_with_source_size(2 * 1024 * 1024 + 1);
        let snapshot = parse_archive(&bytes).unwrap();
        assert_eq!(snapshot.files["src/large.ts"].len(), 2 * 1024 * 1024 + 1);
    }

    #[test]
    fn accepts_source_file_at_fifty_mib_limit() {
        let bytes = archive_with_source_size(50 * 1024 * 1024);
        let snapshot = parse_archive(&bytes).unwrap();
        assert_eq!(snapshot.files["src/large.ts"].len(), 50 * 1024 * 1024);
    }

    #[test]
    fn rejects_source_file_over_limit_without_returning_partial_snapshot() {
        let bytes = archive_with_source_size(50 * 1024 * 1024 + 1);
        assert!(parse_archive(&bytes).unwrap_err().contains("50 MiB"));
    }
}
