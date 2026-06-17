# rusttp

A fast, user-friendly HTTP client for the terminal — httpie rewritten in Rust.

## Features

- **Blazingly Fast** — Starts instantly compared to python-based httpie
- **Syntax Highlighting** — Automatic colorization for JSON, HTML, and XML response bodies
- **Intuitive Syntax** — Simple syntax for headers, query parameters, form fields, and raw JSON values
- **JSON Support** — Automatic serialization and pretty-printing of JSON payloads
- **Form Data & File Uploads** — Easily send multipart form data and file attachments
- **Download Mode** — Download large files with an interactive progress bar
- **Offline Mode** — Preview your requests before sending them

## Installation

```bash
cargo install --path .
```

## Usage

### Simple GET Request

```bash
rusttp GET https://httpbin.org/get
```

### POST with JSON Body

```bash
rusttp POST https://httpbin.org/post name="Anilcan Kara" age:=28
```

- `name="Anilcan Kara"` sends a string field
- `age:=28` sends a raw JSON number

### Request with Headers and Query Parameters

```bash
rusttp https://api.github.com/users/anilcan-kara X-Header:value page==2
```

- `X-Header:value` adds an HTTP header
- `page==2` adds a query parameter

### Form and File Upload

```bash
rusttp --form POST https://httpbin.org/post file@./photo.jpg
```

### Offline Mode (Preview Request)

```bash
rusttp --offline POST https://httpbin.org/post name=Anilcan
```

## License

This project is licensed under the MIT License.
