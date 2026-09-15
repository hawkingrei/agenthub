use super::*;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn membership() -> Value {
    json!({"workspace_id":"cd270331-80bc-4f90-8cc0-3fefbc7f74ab",
        "key_scope":{"scope_mode":"narrowed","grants":["space-a"],"write_space":"space-a"},
        "key_write_target":{"write_space":"space-a","write_space_live":true},
        "display_name":"private-member-name"})
}

#[test]
fn membership_requires_exact_single_space_authority_and_active_placement() {
    let valid = membership();
    assert_eq!(validate(&valid, "space-a").unwrap(), valid["workspace_id"]);
    let mut personal = valid.clone();
    personal["key_scope"]["write_space"] = Value::Null;
    assert!(validate(&personal, "space-a").is_ok());
    for (path, value) in [
        ("/workspace_id", json!("invalid")),
        (
            "/workspace_id",
            json!("00000000-0000-0000-0000-000000000000"),
        ),
        ("/key_scope", Value::Null),
        ("/key_scope/scope_mode", json!("full")),
        ("/key_scope/grants", json!([])),
        ("/key_scope/grants", json!(["space-a", "space-b"])),
        ("/key_scope/grants", json!(["space-a", "space-a"])),
        ("/key_scope/grants", json!(["space-b"])),
        ("/key_scope/grants", json!(["SPACE-A"])),
        ("/key_scope/grants", json!("space-a")),
        ("/key_scope/write_space", json!("space-b")),
        ("/key_write_target", Value::Null),
        ("/key_write_target/write_space", json!("space-b")),
        ("/key_write_target/write_space_live", json!(false)),
        ("/key_write_target/write_space_live", json!("true")),
    ] {
        let mut invalid = valid.clone();
        *invalid.pointer_mut(path).unwrap() = value;
        let error = validate(&invalid, "space-a").unwrap_err().to_string();
        assert!(!error.contains("private"), "{path}");
    }
}

async fn serve_once(response: String) -> (Url, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = Url::parse(&format!(
        "http://{}/service/mcp",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let mut chunk = [0; 1024];
            let size = socket.read(&mut chunk).await.unwrap();
            assert!(size > 0 && request.len() + size <= 16_384);
            request.extend_from_slice(&chunk[..size]);
        }
        let request = String::from_utf8(request).unwrap();
        assert!(request.starts_with("GET /service/members/me HTTP/1.1\r\n"));
        assert!(request.contains("authorization: Bearer private-key\r\n"));
        // The probe does not carry any model request or business arguments.
        assert!(request.ends_with("\r\n\r\n"));
        let _ = socket.write_all(response.as_bytes()).await;
    });
    (endpoint, server)
}

fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    let mut key = reqwest::header::HeaderValue::from_static("Bearer private-key");
    key.set_sensitive(true);
    headers.insert(reqwest::header::AUTHORIZATION, key);
    headers
}

#[tokio::test]
async fn membership_probe_binds_the_same_credential_and_bounds_untrusted_responses() {
    for (status, extra, body, permitted) in [
        ("200 OK", "", membership().to_string(), true),
        ("403 Forbidden", "", "private-denial-body".into(), false),
        ("404 Not Found", "", "private-local-server".into(), false),
        ("200 OK", "", "private-invalid-json".into(), false),
        ("200 OK", "Content-Length: 65537\r\n", "".into(), false),
        // Without Content-Length, the streaming bound must still reject the body.
        ("200 OK", "", "x".repeat(MAX_AUTHORIZATION_BYTES + 1), false),
    ] {
        let wire = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\n{extra}\r\n{body}"
        );
        let (endpoint, server) = serve_once(wire).await;
        let result = verify(&endpoint, &headers(), "space-a").await;
        assert_eq!(result.is_ok(), permitted);
        if let Err(error) = result {
            let error = error.to_string();
            for private in ["private-", "127.0.0.1", "/service", "display_name"] {
                assert!(!error.contains(private));
            }
        }
        server.await.unwrap();
    }
}

#[tokio::test]
async fn membership_probe_never_follows_a_redirect_with_its_credential() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let wire = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{}/private-redirect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        target.local_addr().unwrap()
    );
    let (endpoint, server) = serve_once(wire).await;
    assert!(verify(&endpoint, &headers(), "space-a").await.is_err());
    server.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), target.accept())
            .await
            .is_err()
    );
}
