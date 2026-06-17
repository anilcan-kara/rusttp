mod formatter;
mod request;
mod output;

use std::process;
use clap::Parser;
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(
    name = "rusttp",
    version,
    about = "A fast, user-friendly HTTP client for the terminal",
    long_about = "rusttp is a modern HTTP client — like httpie, but blazingly fast.\n\nExamples:\n  rusttp GET https://httpbin.org/get\n  rusttp POST https://httpbin.org/post name=Anilcan age:=28\n  rusttp https://api.github.com/users/anilcan-kara\n  rusttp PUT https://httpbin.org/put X-Token:secret data=value\n  rusttp --form POST https://httpbin.org/post file@./photo.jpg",
    after_help = "Request items:\n  key=value      JSON string field\n  key:=value     Raw JSON value (number, bool, array, object)\n  key==value     URL query parameter\n  header:value   HTTP header\n  key@file       File upload (multipart)"
)]
struct Args {
    #[arg(help = "HTTP method (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS). Defaults to GET, or POST when data is provided.")]
    method_or_url: String,

    #[arg(help = "URL (required if method is given)")]
    url: Option<String>,

    #[arg(help = "Request items: key=value, key:=json, key==query, header:value, key@file")]
    items: Vec<String>,

    #[arg(short = 'j', long, help = "Force JSON content type")]
    json: bool,

    #[arg(short = 'f', long, help = "Serialize data as form fields")]
    form: bool,

    #[arg(long, help = "Force multipart form data")]
    multipart: bool,

    #[arg(short = 'a', long, help = "Credentials (user:password or token)")]
    auth: Option<String>,

    #[arg(long, default_value = "basic", help = "Auth type: basic, bearer")]
    auth_type: String,

    #[arg(short = 'v', long, help = "Verbose output: print request and response")]
    verbose: bool,

    #[arg(long, help = "Print only response headers")]
    headers: bool,

    #[arg(short = 'b', long, help = "Print only response body")]
    body: bool,

    #[arg(short = 'p', long, help = "What to print: H (request headers), B (request body), h (response headers), b (response body)")]
    print: Option<String>,

    #[arg(short = 'F', long, help = "Follow redirects")]
    follow: bool,

    #[arg(long, help = "Maximum number of redirects", default_value = "30")]
    max_redirects: usize,

    #[arg(long, help = "Connection timeout in seconds", default_value = "30")]
    timeout: u64,

    #[arg(long, help = "Do not verify SSL certificates")]
    verify_no: bool,

    #[arg(short = 'd', long, help = "Download the response body to a file")]
    download: bool,

    #[arg(short = 'o', long, help = "Output file path (for --download)")]
    output: Option<String>,

    #[arg(long, help = "Do not format/colorize output")]
    raw: bool,

    #[arg(long, default_value = "monokai", help = "Color style: monokai, github, dracula")]
    style: String,

    #[arg(long, help = "Set a proxy (http://host:port)")]
    proxy: Option<String>,

    #[arg(long, help = "Print the request without sending it")]
    offline: bool,
}

fn resolve_method_and_url(args: &Args) -> (String, String) {
    let known_methods = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];
    let upper = args.method_or_url.to_uppercase();

    if known_methods.contains(&upper.as_str()) {
        let url = args.url.clone().unwrap_or_else(|| {
            eprintln!("{}", "error: URL is required when method is specified");
            process::exit(1);
        });
        (upper, url)
    } else {
        let has_data = args.items.iter().any(|i| {
            i.contains('=') && !i.contains("==") && !i.contains(':')
                || i.contains(":=")
                || i.contains('@')
        });
        let method = if has_data || args.form || args.multipart { "POST" } else { "GET" };
        (method.to_string(), args.method_or_url.clone())
    }
}

fn expand_url(raw: &str) -> String {
    let s = raw.trim();
    if s.starts_with("http://") || s.starts_with("https://") {
        return s.to_string();
    }
    if s.starts_with(':') {
        return format!("http://localhost{}", s);
    }
    if s.starts_with("localhost") || s.starts_with("127.0.0.1") {
        return format!("http://{}", s);
    }
    format!("https://{}", s)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let (method, raw_url) = resolve_method_and_url(&args);
    let url = expand_url(&raw_url);

    let print_flags = if let Some(ref p) = args.print {
        p.clone()
    } else if args.verbose {
        "HBhb".to_string()
    } else if args.headers {
        "h".to_string()
    } else if args.body {
        "b".to_string()
    } else if args.download {
        "h".to_string()
    } else {
        "hb".to_string()
    };

    let parsed = request::ParsedItems::parse(&args.items)?;

    let built_url = request::apply_query_params(&url, &parsed.query_params)?;

    if args.offline {
        output::print_offline_request(
            &method,
            &built_url,
            &parsed,
            &args,
            &print_flags,
        );
        return Ok(());
    }

    let response = request::send_request(
        &method,
        &built_url,
        &parsed,
        &args,
    ).await?;

    let status = response.status();
    let resp_headers = response.headers().clone();
    let resp_version = response.version();

    if args.download {
        output::download_response(response, args.output.as_deref()).await?;
    } else {
        let body_bytes = response.bytes().await?;

        output::print_response(
            status,
            &resp_headers,
            resp_version,
            &body_bytes,
            &print_flags,
            args.raw,
        );
    }

    if !status.is_success() && !status.is_informational() && !status.is_redirection() {
        process::exit(1);
    }

    Ok(())
}
