#[cfg(target_arch = "wasm32")]
use anyhow::anyhow;
use anyhow::Result;
use serde_json::Value;
use url::Url;

#[cfg(not(target_arch = "wasm32"))]
pub type HttpClient = reqwest::Client;

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Default)]
pub struct HttpClient;

pub fn http_client() -> HttpClient {
    #[cfg(not(target_arch = "wasm32"))]
    {
        reqwest::Client::new()
    }
    #[cfg(target_arch = "wasm32")]
    {
        HttpClient
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn get_json(client: &HttpClient, url: Url) -> Result<Value> {
    Ok(client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn get_bytes(client: &HttpClient, url: Url) -> Result<(Vec<u8>, Option<String>)> {
    let response = client.get(url).send().await?.error_for_status()?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = response.bytes().await?.to_vec();
    Ok((bytes, content_type))
}

#[cfg(target_arch = "wasm32")]
pub async fn get_json(_client: &HttpClient, url: Url) -> Result<Value> {
    let text = gloo_net::http::Request::get(url.as_str())
        .send()
        .await?
        .text()
        .await?;
    Ok(serde_json::from_str(&text)?)
}

#[cfg(target_arch = "wasm32")]
pub async fn get_bytes(_client: &HttpClient, url: Url) -> Result<(Vec<u8>, Option<String>)> {
    let response = gloo_net::http::Request::get(url.as_str()).send().await?;
    let content_type = response.headers().get("content-type");
    let bytes = response.binary().await?;
    Ok((bytes, content_type))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn post_json(client: &HttpClient, url: Url, body: Value) -> Result<Value> {
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    let text = response.text().await?;
    if text.trim().is_empty() {
        Ok(Value::Null)
    } else {
        Ok(serde_json::from_str(&text)?)
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn post_json(_client: &HttpClient, url: Url, body: Value) -> Result<Value> {
    let text = gloo_net::http::Request::post(url.as_str())
        .header("content-type", "application/json")
        .body(body.to_string())?
        .send()
        .await?
        .text()
        .await?;
    if text.trim().is_empty() {
        Ok(Value::Null)
    } else {
        Ok(serde_json::from_str(&text)?)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn post_empty(client: &HttpClient, url: Url) -> Result<()> {
    client.post(url).send().await?.error_for_status()?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub async fn post_empty(_client: &HttpClient, url: Url) -> Result<()> {
    let response = gloo_net::http::Request::post(url.as_str()).send().await?;
    if !response.ok() {
        return Err(anyhow!("POST {url} failed with {}", response.status()));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn delete(client: &HttpClient, url: Url) -> Result<()> {
    client.delete(url).send().await?.error_for_status()?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub async fn delete(_client: &HttpClient, url: Url) -> Result<()> {
    let response = gloo_net::http::Request::delete(url.as_str()).send().await?;
    if !response.ok() {
        return Err(anyhow!("DELETE {url} failed with {}", response.status()));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn post_bytes(
    client: &HttpClient,
    url: Url,
    filename: &str,
    bytes: Vec<u8>,
    content_type: &str,
) -> Result<Value> {
    Ok(client
        .post(url)
        .query(&[("filename", filename)])
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .body(bytes)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?)
}
