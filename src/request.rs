use std::collections::HashMap;
use anyhow::{Context, Result, bail};
use reqwest::{Client, Response, Method, header};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use url::Url;

use crate::Args;

#[derive(Debug, Default)]
pub struct ParsedItems {
    pub headers: HashMap<String, String>,
    pub json_fields: Vec<(String, serde_json::Value)>,
    pub query_params: Vec<(String, String)>,
    pub files: Vec<(String, String)>,
}

impl ParsedItems {
    pub fn parse(items: &[String]) -> Result<Self> {
        let mut parsed = ParsedItems::default();

        for item in items {
            if let Some(pos) = item.find(":=") {
                let key = item[..pos].to_string();
                let val_str = &item[pos + 2..];
                let val: serde_json::Value = serde_json::from_str(val_str)
                    .with_context(|| format!("Invalid JSON value for key '{}': {}", key, val_str))?;
                parsed.json_fields.push((key, val));
            } else if let Some(pos) = item.find("==") {
                let key = item[..pos].to_string();
                let val = item[pos + 2..].to_string();
                parsed.query_params.push((key, val));
            } else if let Some(pos) = item.find('@') {
                if pos > 0 && !item[..pos].contains(':') && !item[..pos].contains('=') {
                    let key = item[..pos].to_string();
                    let path = item[pos + 1..].to_string();
                    parsed.files.push((key, path));
                } else {
                    try_parse_header_or_field(item, &mut parsed)?;
                }
            } else if let Some(pos) = item.find('=') {
                if pos > 0 && !item[..pos].contains(':') {
                    let key = item[..pos].to_string();
                    let val = item[pos + 1..].to_string();
                    parsed.json_fields.push((key, serde_json::Value::String(val)));
                } else {
                    try_parse_header_or_field(item, &mut parsed)?;
                }
            } else if let Some(pos) = item.find(':') {
                let key = item[..pos].to_string();
                let val = item[pos + 1..].trim().to_string();
                parsed.headers.insert(key, val);
            } else {
                bail!("Cannot parse request item: {}", item);
            }
        }

        Ok(parsed)
    }
}

fn try_parse_header_or_field(item: &str, parsed: &mut ParsedItems) -> Result<()> {
    if let Some(pos) = item.find(':') {
        let key = item[..pos].to_string();
        let val = item[pos + 1..].trim().to_string();
        parsed.headers.insert(key, val);
    } else {
        bail!("Cannot parse request item: {}", item);
    }
    Ok(())
}

pub fn apply_query_params(url_str: &str, params: &[(String, String)]) -> Result<String> {
    if params.is_empty() {
        return Ok(url_str.to_string());
    }
    let mut url = Url::parse(url_str).context("Invalid URL")?;
    {
        let mut query = url.query_pairs_mut();
        for (k, v) in params {
            query.append_pair(k, v);
        }
    }
    Ok(url.to_string())
}

pub async fn send_request(
    method: &str,
    url: &str,
    parsed: &ParsedItems,
    args: &Args,
) -> Result<Response> {
    let method = Method::from_bytes(method.as_bytes())
        .context("Invalid HTTP method")?;

    let mut client_builder = Client::builder()
        .timeout(std::time::Duration::from_secs(args.timeout));

    if args.follow {
        client_builder = client_builder.redirect(reqwest::redirect::Policy::limited(args.max_redirects));
    } else {
        client_builder = client_builder.redirect(reqwest::redirect::Policy::none());
    }

    if args.verify_no {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }

    if let Some(ref proxy_url) = args.proxy {
        let proxy = reqwest::Proxy::all(proxy_url)
            .context("Invalid proxy URL")?;
        client_builder = client_builder.proxy(proxy);
    }

    let client = client_builder.build().context("Failed to build HTTP client")?;

    let mut req = client.request(method, url);

    req = req.header(header::USER_AGENT, format!("rusttp/{}", env!("CARGO_PKG_VERSION")));

    for (key, val) in &parsed.headers {
        req = req.header(key.as_str(), val.as_str());
    }

    if let Some(ref auth) = args.auth {
        match args.auth_type.to_lowercase().as_str() {
            "bearer" => {
                req = req.header(header::AUTHORIZATION, format!("Bearer {}", auth));
            }
            _ => {
                let encoded = BASE64.encode(auth.as_bytes());
                req = req.header(header::AUTHORIZATION, format!("Basic {}", encoded));
            }
        }
    }

    if args.form || args.multipart {
        if args.multipart || !parsed.files.is_empty() {
            let mut form = reqwest::multipart::Form::new();
            for (key, val) in &parsed.json_fields {
                if let serde_json::Value::String(s) = val {
                    form = form.text(key.clone(), s.clone());
                } else {
                    form = form.text(key.clone(), val.to_string());
                }
            }
            for (key, path) in &parsed.files {
                let file_bytes = tokio::fs::read(path).await
                    .with_context(|| format!("Cannot read file: {}", path))?;
                let filename = std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let mime = mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string();
                let part = reqwest::multipart::Part::bytes(file_bytes)
                    .file_name(filename)
                    .mime_str(&mime)?;
                form = form.part(key.clone(), part);
            }
            req = req.multipart(form);
        } else {
            let mut form_data: Vec<(String, String)> = Vec::new();
            for (key, val) in &parsed.json_fields {
                if let serde_json::Value::String(s) = val {
                    form_data.push((key.clone(), s.clone()));
                } else {
                    form_data.push((key.clone(), val.to_string()));
                }
            }
            req = req.form(&form_data);
        }
    } else if !parsed.json_fields.is_empty() {
        let mut json_obj = serde_json::Map::new();
        for (key, val) in &parsed.json_fields {
            json_obj.insert(key.clone(), val.clone());
        }
        req = req.json(&serde_json::Value::Object(json_obj));
    }

    let response = req.send().await
        .context("Request failed")?;

    Ok(response)
}
