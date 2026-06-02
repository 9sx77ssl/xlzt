use serde_json::Value;
use std::time::Duration;

const ENDPOINT: &str = "https://y7v.lol/api/upload/batch";

pub async fn upload(images: Vec<(Vec<u8>, String)>) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("lzt/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| "x_x".to_string())?;

    let mut form = reqwest::multipart::Form::new();
    for (i, (bytes, mime)) in images.into_iter().enumerate() {
        let ext = match mime.rsplit('/').next().unwrap_or("png") {
            "jpeg" => "jpg",
            "svg+xml" => "svg",
            e => e,
        };
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(format!("clip{i}.{ext}"))
            .mime_str(&mime)
            .map_err(|_| "bad image type".to_string())?;
        form = form.part("file", part);
    }

    let resp = client
        .post(ENDPOINT)
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "upload timeout".to_string()
            } else {
                "cdn unreachable".to_string()
            }
        })?;

    let v: Value = resp.json().await.map_err(|_| "bad cdn response".to_string())?;
    let files = v
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| "upload failed".to_string())?;

    let mut urls = Vec::with_capacity(files.len());
    for f in files {
        match f.get("url").and_then(Value::as_str) {
            Some(u) => urls.push(u.to_string()),
            None => {
                return Err(f
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("upload failed")
                    .to_string())
            }
        }
    }
    Ok(urls)
}
