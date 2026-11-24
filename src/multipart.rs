//! multipart/form-data request bodies for file uploads.
//!
//! `-F name=value` stays JSON (typed fields, existing behavior). Adding a
//! field whose value is `@path` (`-F file=@doc.pdf`) switches the request to
//! multipart: plain fields become text parts, `@path` fields become file
//! parts (filename + content type guessed from the extension).

use std::fs;
use std::path::Path;

pub enum Field {
    Text { name: String, value: String },
    File { name: String, path: String },
}

pub struct Multipart {
    pub content_type: String,
    pub body: Vec<u8>,
}

pub fn build(boundary: &str, fields: &[Field]) -> Result<Multipart, String> {
    let mut body: Vec<u8> = Vec::new();
    for f in fields {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        match f {
            Field::Text { name, value } => {
                body.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name).as_bytes(),
                );
                body.extend_from_slice(value.as_bytes());
                body.extend_from_slice(b"\r\n");
            }
            Field::File { name, path } => {
                let filename = Path::new(path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| format!("bad file path '{}'", path))?
                    .to_string();
                let bytes =
                    fs::read(path).map_err(|e| format!("cannot read '{}': {}", path, e))?;
                body.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                        name, filename
                    )
                    .as_bytes(),
                );
                body.extend_from_slice(
                    format!("Content-Type: {}\r\n\r\n", content_type(&filename)).as_bytes(),
                );
                body.extend_from_slice(&bytes);
                body.extend_from_slice(b"\r\n");
            }
        }
    }
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    Ok(Multipart {
        content_type: format!("multipart/form-data; boundary={}", boundary),
        body,
    })
}

/// DeepSeek judges file format by content, so the header only needs to be
/// reasonable; unknown extensions default to octet-stream.
fn content_type(filename: &str) -> &'static str {
    let lower = filename.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "wav" => "audio/wav",
        "ogg" | "opus" => "audio/ogg",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
}
