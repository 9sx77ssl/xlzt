use serde_json::Value;
use std::time::Duration;

const ENDPOINT: &str = "https://y7v.lol/api/upload/batch";

pub async fn upload(images: Vec<(Vec<u8>, String)>) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("lzt/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| "x_x".to_string())?;

    let mut attempt = 1;
    let resp = loop {
        let mut form = reqwest::multipart::Form::new();
        for (i, (bytes, mime)) in images.iter().enumerate() {
            let ext = match mime.rsplit('/').next().unwrap_or("png") {
                "jpeg" => "jpg",
                e => e,
            };
            let part = reqwest::multipart::Part::bytes(bytes.clone())
                .file_name(format!("clip{i}.{ext}"))
                .mime_str(mime)
                .map_err(|_| "bad image type".to_string())?;
            form = form.part("file", part);
        }

        match client.post(ENDPOINT).multipart(form).send().await {
            Ok(r) => break r,
            Err(e) if (e.is_connect() || e.is_timeout()) && attempt < 3 => {
                attempt += 1;
            }
            Err(e) => {
                return Err(if e.is_timeout() {
                    "upload timeout".to_string()
                } else {
                    "cdn unreachable".to_string()
                });
            }
        }
    };

    match resp.status().as_u16() {
        413 => return Err("image too large".to_string()),
        429 => return Err("cdn busy".to_string()),
        _ => {}
    }

    let v: Value = resp.json().await.map_err(|_| "bad cdn response".to_string())?;
    let files = v
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| "upload failed".to_string())?;

    let urls: Vec<String> = files
        .iter()
        .filter_map(|f| f.get("url").and_then(Value::as_str).map(str::to_string))
        .collect();

    if urls.is_empty() {
        return Err(files
            .first()
            .and_then(|f| f.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("upload failed")
            .to_string());
    }

    Ok(urls)
}
