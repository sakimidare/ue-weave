use crate::{
    error::{AssetError, RecoverError, Result},
    log::Logger,
    model::Ir,
    network::{self, Policy},
};
use reqwest::blocking::Client;
use std::{collections::HashMap, fs, io::Write, path::Path};
use url::Url;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        net::TcpListener,
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn downloads_deduplicates_and_preserves_existing_files() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut requests = 0;
            while requests < 4 && Instant::now() < deadline {
                if let Ok((mut stream, _)) = listener.accept() {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut request = [0; 2048];
                    let size = stream.read(&mut request).unwrap();
                    let request = String::from_utf8_lossy(&request[..size]);
                    let (status, body) = if request.contains("/missing.pdf") {
                        ("404 Not Found", "missing")
                    } else if request.contains("/b/a.pdf") {
                        ("200 OK", "second")
                    } else {
                        ("200 OK", "first")
                    };
                    write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                    requests += 1;
                } else {
                    thread::sleep(Duration::from_millis(5));
                }
            }
            requests
        });
        let output = std::env::temp_dir().join(format!(
            "ue-download-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(output.join("attachments")).unwrap();
        fs::write(output.join("attachments/0000-a.pdf"), b"existing").unwrap();
        let html = format!("<div class='Article_Content'><p><a href='{base}/a.pdf'>A</a><a href='{base}/a.pdf'>Again</a><a href='{base}/b/a.pdf'>B</a><a href='{base}/missing.pdf'>Missing</a></p><table><tr><td><a href='{base}/a.pdf'>Table attachment</a></td></tr></table></div>");
        let html = html
            .replace("</td>", &format!("<img src='{base}/photo.png'></td>"))
            .replace("</p>", &format!("<img src='{base}/photo.png'></p>"));
        let mut ir = crate::parser::parse(
            &scraper::Html::parse_document(&html),
            ".Article_Content",
            false,
            None,
        )
        .unwrap();
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let errors = download_attachments(
            &mut ir,
            &output,
            &client,
            Policy::default(),
            &Logger::new(false),
        )
        .unwrap();
        let image_errors = download_images(
            &mut ir,
            &output,
            &client,
            Policy::default(),
            &Logger::new(false),
        )
        .unwrap();
        assert!(image_errors.is_empty());
        assert_eq!(server.join().unwrap(), 4);
        assert_eq!(ir.images.len(), 2);
        assert_eq!(ir.images[0]["src"], ir.images[1]["src"]);
        let local_image = ir.images[0]["src"].as_str().unwrap();
        assert!(local_image.starts_with("images/"));
        assert_eq!(fs::read(output.join(local_image)).unwrap(), b"first");
        assert_eq!(
            ir.blocks[1].rows[0][0].runs[1].image.as_deref(),
            Some(local_image)
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["code"], "http_status");
        assert_eq!(ir.attachments.len(), 5);
        assert_eq!(
            ir.blocks[1].rows[0][0].runs[0].link.as_deref(),
            ir.attachments[0]["href"].as_str()
        );
        assert_eq!(ir.attachments[0]["href"], ir.attachments[1]["href"]);
        assert_ne!(ir.attachments[0]["href"], ir.attachments[2]["href"]);
        assert_eq!(
            fs::read(output.join("attachments/0000-a.pdf")).unwrap(),
            b"existing"
        );
        assert_eq!(
            fs::read(output.join(ir.attachments[0]["href"].as_str().unwrap())).unwrap(),
            b"first"
        );
        assert_eq!(
            fs::read(output.join(ir.attachments[2]["href"].as_str().unwrap())).unwrap(),
            b"second"
        );
        assert_eq!(
            ir.blocks[0].runs[3].link.as_deref(),
            Some(format!("{base}/missing.pdf").as_str())
        );
        assert!(crate::renderer::render(&ir).contains("href=\"attachments/"));
        fs::remove_dir_all(&output).unwrap();
    }
}

fn file_name(url: &str, index: usize) -> String {
    let candidate = Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .map(str::to_string)
        })
        .unwrap_or_default();
    let candidate = candidate.split('?').next().unwrap_or("");
    let safe: String = candidate
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        format!("attachment-{index:03}.bin")
    } else {
        format!("{index:04}-{}", safe.chars().take(160).collect::<String>())
    }
}

pub fn download_attachments(
    ir: &mut Ir,
    output: &Path,
    client: &Client,
    policy: Policy,
    logger: &Logger,
) -> Result<Vec<serde_json::Value>> {
    download_assets(ir, output, client, false, policy, logger)
}

pub fn download_images(
    ir: &mut Ir,
    output: &Path,
    client: &Client,
    policy: Policy,
    logger: &Logger,
) -> Result<Vec<serde_json::Value>> {
    download_assets(ir, output, client, true, policy, logger)
}

fn download_assets(
    ir: &mut Ir,
    output: &Path,
    client: &Client,
    images: bool,
    policy: Policy,
    logger: &Logger,
) -> Result<Vec<serde_json::Value>> {
    let folder = if images { "images" } else { "attachments" };
    let field = if images { "src" } else { "href" };
    let directory = output.join(folder);
    fs::create_dir_all(&directory).map_err(|source| RecoverError::CreateOutput {
        path: directory.clone(),
        source,
    })?;
    let mut errors = Vec::new();
    let mut downloaded: HashMap<String, String> = HashMap::new();
    let assets = if images {
        &mut ir.images
    } else {
        &mut ir.attachments
    };
    for (index, attachment) in assets.iter_mut().enumerate() {
        let Some(url) = attachment
            .get(field)
            .and_then(|value| value.as_str())
            .map(str::to_string)
        else {
            continue;
        };
        let result = if let Some(local) = downloaded.get(&url) {
            Ok(local.clone())
        } else {
            fetch_attachment(client, &url, &directory, index, images, policy, logger)
                .map(|name| format!("{folder}/{name}"))
        };
        match result {
            Ok(local) => {
                downloaded.insert(url.clone(), local.clone());
                attachment[format!("remote_{field}")] = url.clone().into();
                attachment[field] = local.clone().into();
                attachment["local_path"] = local.clone().into();
                for block in &mut ir.blocks {
                    block.visit_runs_mut(&mut |run| {
                        let target = if images { &mut run.image } else { &mut run.link };
                        if target.as_deref() == Some(&url) {
                            *target = Some(local.clone());
                        }
                    });
                }
            }
            Err(error) => errors
                .push(serde_json::json!({"kind":if images {"image-download"} else {"attachment-download"},"url":url,"code":error.code(),"error":error.to_string(),"network":match &error { AssetError::Network(error) => error.details(), _ => serde_json::Value::Null }})),
        }
    }
    Ok(errors)
}

fn fetch_attachment(
    client: &Client,
    url: &str,
    directory: &Path,
    index: usize,
    image: bool,
    policy: Policy,
    logger: &Logger,
) -> std::result::Result<String, AssetError> {
    const LIMIT: u64 = 64 * 1024 * 1024;
    let parsed = Url::parse(url)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AssetError::Scheme);
    }
    let bytes = network::fetch(client, url, policy, Some(LIMIT), logger)?.bytes;
    let mut name = file_name(url, index);
    if image {
        let extension = if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            Some("jpg")
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some("png")
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            Some("gif")
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            Some("webp")
        } else {
            None
        };
        if let Some(extension) = extension {
            let mut path = std::path::PathBuf::from(&name);
            path.set_extension(extension);
            name = path.to_string_lossy().into_owned();
        }
    }
    for suffix in 0..10000 {
        let name = if suffix == 0 {
            name.clone()
        } else {
            format!("{suffix}-{name}")
        };
        let path = directory.join(&name);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(&bytes) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(AssetError::Write {
                        path,
                        source: error,
                    });
                }
                return Ok(name);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(AssetError::Write {
                    path,
                    source: error,
                })
            }
        }
    }
    Err(AssetError::NameExhausted)
}
