use serde_json::{json, Value};
use std::time::Duration;

const HOSTS: [&str; 4] = [
    "https://prod-api.lolz.live",
    "https://prod-api.zelenka.guru",
    "https://api.lolz.live",
    "https://api.zelenka.guru",
];

pub async fn create_thread(
    token: &str,
    forum_id: u64,
    title: &str,
    body: &str,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("lzt/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "x_x".to_string())?;

    let payload = json!({ "forum_id": forum_id, "title": title, "post_body": body });
    let mut last = "x_x".to_string();

    for host in HOSTS {
        match client
            .post(format!("{host}/threads"))
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return permalink(resp).await;
                }
                if status.is_server_error() {
                    last = format!("{} ^.^", status.as_u16());
                    continue;
                }
                return Err(format!("{} ^.^", status.as_u16()));
            }
            Err(e) if e.is_timeout() || e.is_connect() => {
                last = "x_x".to_string();
                continue;
            }
            Err(_) => return Err("x_x".to_string()),
        }
    }
    Err(last)
}

async fn permalink(resp: reqwest::Response) -> Result<String, String> {
    let v: Value = resp.json().await.map_err(|_| "x_x".to_string())?;
    v.get("thread")
        .and_then(|t| t.get("links"))
        .and_then(|l| l.get("permalink"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "x_x".to_string())
}
