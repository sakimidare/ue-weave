use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
fn private_network_requires_explicit_option() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let dir = std::env::temp_dir().join(format!("ue-private-network-{}", std::process::id()));
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg(format!("http://{}", listener.local_addr().unwrap()))
        .arg("--output")
        .arg(&dir)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!result.status.success());
    let error: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(error["code"], "E0109");
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

fn serve(responses: Vec<(u16, String)>) -> (String, thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut count = 0;
        for (status, body) in responses {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                if Instant::now() > deadline {
                    return count;
                }
                thread::sleep(Duration::from_millis(5));
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let _ = stream.read(&mut [0; 4096]);
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            count += 1;
        }
        count
    });
    (url, server)
}

#[test]
fn page_and_attachment_retry_and_local_config_use_same_pipeline() {
    let page = "<h1>本地站点标题</h1><section class='body'><p><span class='wp_pdf_player' pdfsrc='/file.pdf' \
        sudyfile-attr=\"{'title':'通知.pdf'}\"></span></p></section>";
    let (url, server) = serve(vec![
        (503, "".into()),
        (200, page.into()),
        (503, "".into()),
        (200, "PDF test bytes".into()),
    ]);
    let dir = std::env::temp_dir().join(format!("ue-http-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let config = dir.join("sites.json");
    fs::write(&config, r#"{"content":".body","title":"h1"}"#).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg(&url)
        .args(["--formats", "html,markdown,typst,json"])
        .arg("--allow-private-network")
        .args([
            "--download-attachments",
            "--retries",
            "1",
            "--timeout",
            "2",
            "--json",
            "--verbose",
            "--site-config",
        ])
        .arg(&config)
        .arg("--output")
        .arg(dir.join("out"))
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(server.join().unwrap(), 4);
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["selector"], ".body");
    assert!(report["attachment_errors"].as_array().unwrap().is_empty());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr)
            .matches("| retry:")
            .count(),
        2
    );
    let ir: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("out/reconstructed.json")).unwrap()).unwrap();
    assert_eq!(ir["metadata"]["title"], "本地站点标题");
    let local = ir["attachments"][0]["href"].as_str().unwrap();
    assert_eq!(
        fs::read(dir.join("out").join(local)).unwrap(),
        b"PDF test bytes"
    );
    for file in [
        "reconstructed.html",
        "reconstructed.md",
        "reconstructed.typ",
    ] {
        assert!(fs::read_to_string(dir.join("out").join(file))
            .unwrap()
            .contains(local));
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn exhausted_retries_produce_structured_error() {
    let (url, server) = serve(vec![(503, "".into()), (503, "".into())]);
    let dir = std::env::temp_dir().join(format!("ue-http-error-{}", std::process::id()));
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg(&url)
        .args([
            "--allow-private-network",
            "--retries",
            "1",
            "--timeout",
            "2",
            "--json",
            "--output",
        ])
        .arg(&dir)
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(server.join().unwrap(), 2);
    let error: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(error["code"], "E0106");
    assert_eq!(error["network"]["http_status"], 503);
    assert_eq!(error["network"]["attempts"], 2);
    assert_eq!(error["network"]["url"], format!("{url}/"));
    assert!(!dir.join("reconstructed.html").exists());
}

#[test]
fn custom_headers_are_sent_to_the_page_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = String::new();
        let mut buffer = [0u8; 4096];
        let size = stream.read(&mut buffer).unwrap();
        request.push_str(&String::from_utf8_lossy(&buffer[..size]));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("referer: https://example.edu/"),
            "{request}"
        );
        assert!(request.contains("x-test: accepted"), "{request}");
        let body = "<div class='body'><p>正文</p></div>";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let dir = std::env::temp_dir().join(format!("ue-http-header-{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg(&url)
        .args([
            "--allow-private-network",
            "--header",
            "Referer=https://example.edu/",
        ])
        .args([
            "--header",
            "X-Test=accepted",
            "--selector",
            ".body",
            "--output",
        ])
        .arg(&dir)
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_header_configuration_has_dedicated_error_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args([
            "http://example.org/",
            "--header",
            "Host=bad",
            "--output",
            "/tmp/ue-invalid-header",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0014"), "{stderr}");
}

#[test]
fn headers_file_errors_use_dedicated_error_code() {
    let file = std::env::temp_dir().join(format!("ue-invalid-headers-{}.json", std::process::id()));
    fs::write(&file, "[]").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["http://example.org/", "--headers-file"])
        .arg(&file)
        .args(["--output", "/tmp/ue-invalid-headers-file"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("E0003"));
    fs::remove_file(file).unwrap();
}
