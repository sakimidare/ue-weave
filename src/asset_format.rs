//! 按文件内容签名检查扩展名，未知类型保留原文件名。
use crate::error::AssetError;

pub struct Format {
    pub extension: String,
    pub detected_type: &'static str,
    pub adjustment: Option<String>,
}

fn mime_extension(mime: &str) -> Option<&'static str> {
    match mime {
        "application/pdf" => Some("pdf"),
        "application/zip" | "application/x-zip-compressed" => Some("zip"),
        "application/msword" => Some("doc"),
        "application/vnd.ms-excel" => Some("xls"),
        "application/vnd.ms-powerpoint" => Some("ppt"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some("xlsx"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
        "text/plain" => Some("txt"),
        "text/csv" => Some("csv"),
        "application/json" => Some("json"),
        _ => None,
    }
}

pub fn choose(
    bytes: &[u8],
    image: bool,
    content_type: Option<&str>,
    filename: &str,
) -> Result<Format, AssetError> {
    let mime = content_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let named = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| value.len() <= 16 && value.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .unwrap_or("")
        .to_ascii_lowercase();
    let proposed = if named.is_empty() {
        mime_extension(&mime).unwrap_or("bin")
    } else {
        &named
    };
    let signature = if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("jpg", "image/jpeg"))
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("png", "image/png"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("gif", "image/gif"))
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(("webp", "image/webp"))
    } else if bytes.starts_with(b"%PDF-") {
        Some(("pdf", "application/pdf"))
    } else if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        Some(("zip", "zip-container"))
    } else if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) {
        Some(("bin", "ole-container"))
    } else {
        None
    };
    if mime == "text/html" || mime == "application/xhtml+xml" {
        return Err(AssetError::ContentType(
            "服务器返回 HTML 页面；请检查资源地址或访问状态".into(),
        ));
    }
    let (extension, detected_type) = if let Some((extension, detected)) = signature {
        if image && !detected.starts_with("image/") {
            return Err(AssetError::ContentType(format!(
                "图片内容类型为 {detected}"
            )));
        }
        if (mime.starts_with("image/") || mime == "application/pdf")
            && mime != detected
            && !(mime.starts_with("image/") && detected.starts_with("image/"))
        {
            return Err(AssetError::ContentType(format!(
                "响应声明 {mime}，文件签名为 {detected}"
            )));
        }
        let extension = match detected {
            "zip-container"
                if [
                    "docx", "xlsx", "pptx", "docm", "xlsm", "pptm", "odt", "ods", "odp", "epub",
                    "zip",
                ]
                .contains(&proposed) =>
            {
                proposed
            }
            "ole-container" if ["doc", "xls", "ppt", "msg"].contains(&proposed) => proposed,
            "image/jpeg" if proposed == "jpeg" => proposed,
            _ => extension,
        };
        (extension, detected)
    } else if image {
        return Err(AssetError::ContentType(
            "图片签名未匹配 JPEG、PNG、GIF 或 WebP".into(),
        ));
    } else if mime == "application/pdf" {
        return Err(AssetError::ContentType(
            "响应声明 PDF，内容缺少 PDF 文件头".into(),
        ));
    } else {
        let active = [
            "exe", "com", "scr", "msi", "msp", "bat", "cmd", "ps1", "psm1", "sh", "bash", "zsh",
            "py", "pyw", "js", "jse", "vbs", "vbe", "hta", "html", "htm", "xhtml", "svg", "lnk",
            "url", "desktop", "jar", "command",
        ];
        let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(512)])
            .trim_start()
            .to_ascii_lowercase();
        let executable = bytes.starts_with(b"MZ")
            || bytes.starts_with(b"\x7fELF")
            || bytes.starts_with(b"#!")
            || prefix.starts_with("<!doctype html")
            || prefix.starts_with("<html")
            || prefix.starts_with("<script")
            || prefix.starts_with("<svg");
        (
            if executable || active.contains(&proposed) {
                "download"
            } else {
                proposed
            },
            "unknown",
        )
    };
    Ok(Format {
        extension: extension.into(),
        detected_type,
        adjustment: if !mime.is_empty() && mime.starts_with("image/") && mime != detected_type {
            Some(format!(
                "响应声明 {mime}，按文件内容 {detected_type} 保存；扩展名 {extension}"
            ))
        } else {
            (extension != proposed).then(|| {
                format!("扩展名从 {proposed} 调整为 {extension}；内容类型 {detected_type}")
            })
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_mime_mismatches_use_the_detected_format() {
        for (bytes, mime, name, ext) in [
            (&b"\xff\xd8\xffimage"[..], "image/png", "image.png", "jpg"),
            (&b"\xff\xd8\xffimage"[..], "image/png", "image.jpg", "jpg"),
            (&b"\x89PNG\r\n\x1a\n"[..], "image/jpeg", "image.jpg", "png"),
        ] {
            let format = choose(bytes, true, Some(mime), name).unwrap();
            assert_eq!(format.extension, ext);
            assert!(format.adjustment.unwrap().contains(mime));
        }
        assert!(choose(b"<html>error</html>", true, Some("image/png"), "a.png").is_err());
        assert!(choose(b"\xff\xd8\xffimage", true, Some("text/html"), "a.jpg").is_err());
    }
    #[test]
    fn preserves_office_containers_and_arbitrary_data_extensions() {
        for ext in ["docx", "xlsx", "pptx", "epub", "zip"] {
            let result = choose(b"PK\x03\x04data", false, None, &format!("file.{ext}")).unwrap();
            assert_eq!(result.extension, ext);
            assert_eq!(result.detected_type, "zip-container");
        }
        let ole = &[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
        assert_eq!(
            choose(ole, false, None, "file.doc").unwrap().extension,
            "doc"
        );
        for ext in ["csv", "txt", "dwg", "mat", "json"] {
            assert_eq!(
                choose(b"data", false, None, &format!("file.{ext}"))
                    .unwrap()
                    .extension,
                ext
            );
        }
        assert_eq!(
            choose(
                b"PK\x03\x04data",
                false,
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
                "download"
            )
            .unwrap()
            .extension,
            "docx"
        );
    }
    #[test]
    fn signatures_and_declared_types_record_conflicts() {
        assert!(choose(b"<html>login</html>", true, Some("image/png"), "file.png").is_err());
        assert!(choose(b"%PDF-1.7", true, None, "file.png").is_err());
        assert!(choose(b"%PDF-1.7", false, Some("text/html"), "file.pdf").is_err());
        let result = choose(b"%PDF-1.7", false, None, "file.doc").unwrap();
        assert_eq!(result.extension, "pdf");
        assert!(result.adjustment.is_some());
        assert_eq!(
            choose(b"MZ executable", false, None, "file.txt")
                .unwrap()
                .extension,
            "download"
        );
        assert_eq!(
            choose(b"\x89PNG\r\n\x1a\n", true, None, "file.jpeg")
                .unwrap()
                .extension,
            "png"
        );
    }
}
