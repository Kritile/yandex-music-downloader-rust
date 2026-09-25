use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

use yandex_music_downloader::api::MusicClient;

fn mock_server(responses: Vec<(u16, &'static str)>) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let header_end = loop {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0, "request ended before headers");
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap_or(0);
            while bytes.len() < header_end + length {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0, "request ended before body");
                bytes.extend_from_slice(&buffer[..count]);
            }
            requests.push(String::from_utf8(bytes).unwrap());
            write!(
                stream,
                "HTTP/1.1 {status} Mock\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
        requests
    });
    (format!("http://{address}"), server)
}

fn client(base_url: &str, tries: u32) -> MusicClient {
    MusicClient::new("fixture-secret".into(), 2, tries, 0, false)
        .unwrap()
        .with_api_base_url(base_url)
        .unwrap()
}

#[test]
fn permits_plain_http_only_for_loopback_api_base() {
    let make_client = || MusicClient::new("fixture-secret".into(), 2, 0, 0, false).unwrap();
    assert!(make_client()
        .with_api_base_url("http://[::1]:12345")
        .is_ok());
    assert!(make_client()
        .with_api_base_url("http://example.com")
        .is_err());
}

#[test]
fn posts_tracks_with_oauth_header_and_parses_result() {
    let (base_url, server) = mock_server(vec![(
        200,
        r#"{"result":[{"id":42,"title":"Example","available":true}]}"#,
    )]);
    let tracks = client(&base_url, 1).tracks(&["42".into()]).unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].id, "42");
    assert_eq!(tracks[0].title, "Example");
    let requests = server.join().unwrap();
    let request = requests[0].to_ascii_lowercase();
    assert!(request.starts_with("post /tracks http/1.1"));
    assert!(request.contains("authorization: oauth fixture-secret"));
    assert!(request.contains("x-yandex-music-client: yandexmusicandroid/24023621"));
    assert!(request.contains("track-ids=42&with-positions=true"));
}

#[test]
fn retries_server_error_then_returns_tracks() {
    let (base_url, server) = mock_server(vec![
        (503, "temporary"),
        (200, r#"{"result":[{"id":"42","title":"Recovered"}]}"#),
    ]);
    let tracks = client(&base_url, 1).tracks(&["42".into()]).unwrap();
    assert_eq!(tracks[0].title, "Recovered");
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn redacts_token_from_http_error_body() {
    let (base_url, server) = mock_server(vec![(401, "bad fixture-secret")]);
    let error = client(&base_url, 1)
        .tracks(&["42".into()])
        .unwrap_err()
        .to_string();
    assert!(error.contains("[redacted]"));
    assert!(!error.contains("fixture-secret"));
    server.join().unwrap();
}
