use colored::*;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::request::ParsedItems;
use crate::formatter;
use crate::Args;

pub fn print_response(
    status: StatusCode,
    headers: &HeaderMap,
    version: reqwest::Version,
    body: &[u8],
    print_flags: &str,
    raw: bool,
) {
    if print_flags.contains('h') {
        let version_str = match version {
            reqwest::Version::HTTP_09 => "HTTP/0.9",
            reqwest::Version::HTTP_10 => "HTTP/1.0",
            reqwest::Version::HTTP_11 => "HTTP/1.1",
            reqwest::Version::HTTP_2 => "HTTP/2",
            reqwest::Version::HTTP_3 => "HTTP/3",
            _ => "HTTP/?",
        };

        let status_line = format!("{} {}", version_str, status);
        if raw {
            println!("{}", status_line);
        } else {
            let color = if status.is_success() {
                "green"
            } else if status.is_redirection() {
                "yellow"
            } else {
                "red"
            };
            match color {
                "green" => println!("{}", status_line.green().bold()),
                "yellow" => println!("{}", status_line.yellow().bold()),
                _ => println!("{}", status_line.red().bold()),
            }
        }

        let mut sorted_headers: Vec<_> = headers.iter().collect();
        sorted_headers.sort_by_key(|(k, _)| k.as_str().to_string());

        for (key, val) in &sorted_headers {
            let val_str = val.to_str().unwrap_or("<binary>");
            if raw {
                println!("{}: {}", key, val_str);
            } else {
                println!("{}{} {}", key.as_str().cyan(), ":".white(), val_str);
            }
        }

        println!();
    }

    if print_flags.contains('b') {
        let body_str = String::from_utf8_lossy(body);

        if body_str.is_empty() {
            return;
        }

        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if raw {
            println!("{}", body_str);
        } else if content_type.contains("json") {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&body_str) {
                let pretty = serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| body_str.to_string());
                formatter::print_highlighted_json(&pretty);
            } else {
                println!("{}", body_str);
            }
        } else if content_type.contains("xml") || content_type.contains("html") {
            formatter::print_highlighted_xml(&body_str);
        } else {
            println!("{}", body_str);
        }
    }
}

pub fn print_offline_request(
    method: &str,
    url: &str,
    parsed: &ParsedItems,
    args: &Args,
    print_flags: &str,
) {
    if print_flags.contains('H') {
        println!("{}", format!("{} {} HTTP/1.1", method, url).green().bold());

        let host = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_default();
        println!("{}{} {}", "Host".cyan(), ":".white(), host);
        println!("{}{} {}", "User-Agent".cyan(), ":".white(), format!("rusttp/{}", env!("CARGO_PKG_VERSION")));

        if !parsed.json_fields.is_empty() && !args.form {
            println!("{}{} {}", "Content-Type".cyan(), ":".white(), "application/json");
            println!("{}{} {}", "Accept".cyan(), ":".white(), "application/json, */*;q=0.5");
        } else if args.form {
            println!("{}{} {}", "Content-Type".cyan(), ":".white(), "application/x-www-form-urlencoded");
        }

        for (key, val) in &parsed.headers {
            println!("{}{} {}", key.cyan(), ":".white(), val);
        }

        println!();
    }

    if print_flags.contains('B') && !parsed.json_fields.is_empty() {
        let mut obj = serde_json::Map::new();
        for (key, val) in &parsed.json_fields {
            obj.insert(key.clone(), val.clone());
        }
        let pretty = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
            .unwrap_or_default();
        formatter::print_highlighted_json(&pretty);
    }
}

pub async fn download_response(
    response: reqwest::Response,
    output_path: Option<&str>,
) -> Result<()> {
    let filename = output_path
        .map(|s| s.to_string())
        .or_else(|| {
            response.headers().get("content-disposition")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| {
                    v.split("filename=").nth(1)
                        .map(|s| s.trim_matches('"').to_string())
                })
        })
        .or_else(|| {
            response.url().path_segments()
                .and_then(|seg| seg.last())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "download".to_string());

    let total_size = response.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("█░░")
    );
    pb.set_message(format!("Downloading {}", filename.green()));

    let bytes = response.bytes().await?;
    pb.set_position(bytes.len() as u64);

    tokio::fs::write(&filename, &bytes).await?;
    pb.finish_with_message(format!("Saved to {} ({} bytes)", filename.green().bold(), bytes.len()));

    Ok(())
}
