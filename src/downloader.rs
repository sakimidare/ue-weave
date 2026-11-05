use crate::diagnostic::{Details, Diagnostic, WarningKind};
use crate::network_policy::Client;
use crate::{
    error::{AssetError, RecoverError, Result},
    log::Logger,
    model::Ir,
    network::{self, Policy},
};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, io::Write, path::Path};
use url::Url;

#[derive(Clone, serde::Serialize)]
struct StoredAsset {
    requested_url: String,
    final_url: String,
    status: u16,
    content_type: Option<String>,
    size: usize,
    sha256: String,
    local_path: String,
    original_filename: String,
    filename_source: String,
    detected_type: String,
    filename_adjustment: Option<String>,
}

struct AssetRequest<'a> {
    url: &'a str,
    directory: &'a Path,
    index: usize,
    image: bool,
    title: Option<&'a str>,
}

/// 下载文章附件并将成功下载的链接替换为本地路径，返回下载诊断。
pub fn download_attachments(
    ir: &mut Ir,
    output: &Path,
    client: &Client,
    policy: Policy,
    logger: &Logger,
) -> Result<Vec<Diagnostic>> {
    download_assets(ir, output, client, false, policy, logger)
}

/// 下载文章图片并更新正文引用，复用传入的客户端与请求设置。
pub fn download_images(
    ir: &mut Ir,
    output: &Path,
    client: &Client,
    policy: Policy,
    logger: &Logger,
) -> Result<Vec<Diagnostic>> {
    download_assets(ir, output, client, true, policy, logger)
}

fn download_assets(
    ir: &mut Ir,
    output: &Path,
    client: &Client,
    images: bool,
    policy: Policy,
    logger: &Logger,
) -> Result<Vec<Diagnostic>> {
    let folder = if images { "images" } else { "attachments" };
    let field = if images { "src" } else { "href" };
    let directory = output.join(folder);
    fs::create_dir_all(&directory).map_err(|source| RecoverError::CreateOutput {
        path: directory.clone(),
        source,
    })?;
    let root = fs::canonicalize(output).map_err(|source| RecoverError::ReadInput {
        path: output.into(),
        source,
    })?;
    let canonical = fs::canonicalize(&directory).map_err(|source| RecoverError::ReadInput {
        path: directory.clone(),
        source,
    })?;
    if !canonical.starts_with(&root) {
        return Err(RecoverError::Arguments(format!(
            "资源目录指向输出包外部：{}",
            directory.display()
        )));
    }
    let mut errors = Vec::new();
    let mut downloaded: HashMap<String, StoredAsset> = HashMap::new();
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
            let title = attachment["attributes"]["sudyfile_json"]["title"]
                .as_str()
                .or_else(|| attachment["text"].as_str());
            fetch_attachment(
                client,
                AssetRequest {
                    url: &url,
                    directory: &directory,
                    index,
                    image: images,
                    title,
                },
                policy,
                logger,
            )
            .map(|mut record| {
                record.local_path = format!("{folder}/{}", record.local_path);
                record
            })
        };
        match result {
            Ok(record) => {
                if record.filename_adjustment.is_some() || record.detected_type == "unknown" {
                    ir.warnings.push(Diagnostic::new(
                        WarningKind::AssetFormat,
                        Details {
                            url: Some((url).to_owned()),
                            original: serde_json::json!(record.original_filename),
                            value: serde_json::json!(record.local_path),
                            detected_type: Some((record.detected_type).to_owned()),
                            reason: Some(
                                (record
                                    .filename_adjustment
                                    .as_deref()
                                    .unwrap_or("文件内容类型记为 unknown，保留来源扩展名"))
                                .to_owned(),
                            ),
                            source_nodes: attachment["source_nodes"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|id| id.as_str().map(str::to_owned))
                                .collect(),
                            ..Default::default()
                        },
                    ));
                }
                let local = record.local_path.clone();
                downloaded.insert(url.clone(), record.clone());
                attachment["download"] = serde_json::json!(record);
                attachment[format!("remote_{field}")] = url.clone().into();
                attachment[field] = local.clone().into();
                attachment["local_path"] = local.clone().into();
                for block in &mut ir.blocks {
                    block.visit_runs_mut(&mut |run| {
                        let target = if images {
                            &mut run.image
                        } else {
                            &mut run.link
                        };
                        if target.as_deref() == Some(&url) {
                            *target = Some(local.clone());
                        }
                    });
                }
            }
            Err(error) => errors.push(Diagnostic::new(
                if images {
                    WarningKind::ImageDownload
                } else {
                    WarningKind::AttachmentDownload
                },
                Details {
                    url: Some((url).to_owned()),
                    error_code: Some((error.code()).to_owned()),
                    error: Some((error.to_string()).to_owned()),
                    network: serde_json::json!(match &error {
                        AssetError::Network(error) => error.details(),
                        _ => serde_json::Value::Null,
                    }),
                    ..Default::default()
                },
            )),
        }
    }
    Ok(errors)
}

fn fetch_attachment(
    client: &Client,
    request: AssetRequest<'_>,
    policy: Policy,
    logger: &Logger,
) -> std::result::Result<StoredAsset, AssetError> {
    const LIMIT: u64 = network::MAX_BODY_SIZE;
    let AssetRequest {
        url,
        directory,
        index,
        image,
        title,
    } = request;
    let parsed = Url::parse(url)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AssetError::Scheme);
    }
    let download = network::fetch(client, url, policy, Some(LIMIT), logger)?;
    let bytes = &download.bytes;
    let (proposed, filename_source) = crate::asset_name::proposed(
        download.content_disposition.as_deref(),
        title,
        &download.url,
    );
    let format =
        crate::asset_format::choose(bytes, image, download.content_type.as_deref(), &proposed)?;
    let name = crate::asset_name::filename(&proposed, &format.extension, index);
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
                if let Err(error) = file.write_all(bytes) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(AssetError::Write {
                        path,
                        source: error,
                    });
                }
                return Ok(StoredAsset {
                    requested_url: url.into(),
                    final_url: download.url,
                    status: download.status,
                    content_type: download.content_type,
                    size: bytes.len(),
                    sha256: format!("{:x}", Sha256::digest(bytes)),
                    local_path: name,
                    original_filename: proposed,
                    filename_source: filename_source.into(),
                    detected_type: format.detected_type.into(),
                    filename_adjustment: format.adjustment,
                });
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
    fn content_disposition_preserves_office_filename_and_hash() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/download?id=1", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let _ = stream.read(&mut [0; 2048]);
            let body = b"PK\x03\x04office-container";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Disposition: attachment; \
                filename*=UTF-8''%E9%80%9A%E7%9F%A5.docx\r\nContent-Type: \
                application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });
        let dir = std::env::temp_dir().join(format!("ue-office-name-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let record = fetch_attachment(
            &crate::network_policy::client(
                Duration::from_secs(2),
                true,
                reqwest::header::HeaderMap::new(),
            )
            .unwrap(),
            AssetRequest {
                url: &url,
                directory: &dir,
                index: 0,
                image: false,
                title: None,
            },
            Policy {
                allow_private_network: true,
                ..Default::default()
            },
            &Logger::new(false),
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(record.local_path, "0000-通知.docx");
        assert_eq!(record.filename_source, "content-disposition");
        assert_eq!(record.detected_type, "zip-container");
        assert_eq!(
            record.sha256,
            format!(
                "{:x}",
                Sha256::digest(fs::read(dir.join(&record.local_path)).unwrap())
            )
        );
        fs::remove_dir_all(dir).unwrap();
    }

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
                    } else if request.contains("/photo.png") {
                        ("200 OK", "GIF89afirst")
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
        let html = format!(
            "<div class='Article_Content'><p><a href='{base}/a.pdf'>A</a><a href='{base}/a.pdf'>Again</a><a \
                href='{base}/b/a.pdf'>B</a><a href='{base}/missing.pdf'>Missing</a></p><table><tr><td><a \
                href='{base}/a.pdf'>Table attachment</a></td></tr></table></div>"
        );
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
        let client: Client = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap()
            .into();
        let errors = download_attachments(
            &mut ir,
            &output,
            &client,
            Policy {
                allow_private_network: true,
                ..Policy::default()
            },
            &Logger::new(false),
        )
        .unwrap();
        let image_errors = download_images(
            &mut ir,
            &output,
            &client,
            Policy {
                allow_private_network: true,
                ..Policy::default()
            },
            &Logger::new(false),
        )
        .unwrap();
        assert!(image_errors.is_empty());
        assert_eq!(server.join().unwrap(), 4);
        assert_eq!(ir.images.len(), 2);
        assert_eq!(ir.images[0]["src"], ir.images[1]["src"]);
        let local_image = ir.images[0]["src"].as_str().unwrap();
        assert!(local_image.starts_with("images/"));
        assert_eq!(fs::read(output.join(local_image)).unwrap(), b"GIF89afirst");
        assert_eq!(
            ir.blocks[1].rows[0][0].runs[1].image.as_deref(),
            Some(local_image)
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["code"], "W0006");
        assert_eq!(errors[0]["error_code"], "E0106");
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
