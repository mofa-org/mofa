//! Vision utilities for multi-modal message construction
//!
//! This module provides:
//! - MIME type detection
//! - Base64 encoding
//! - Structured content array generation
//! - Multi-modal message builders

use crate::llm::types::{ChatMessage, ContentPart, ImageDetail, ImageUrl, MessageContent, Role};
use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use std::path::Path;

/// Encode an image file as a data URL
///
/// # Arguments
/// * `path` - Path to the image file
///
/// # Returns
/// A data URL string in the format `data:<mime-type>;base64,<data>`
///
/// # Example
/// ```ignore
/// let url = encode_image_data_url(Path::new("/path/to/image.png"))?;
/// assert!(url.starts_with("data:image/png;base64,"));
/// ```
pub fn encode_image_data_url(path: &Path) -> GlobalResult<String> {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD_NO_PAD;
    use std::fs;

    let bytes = fs::read(path)?;

    let mime_type = infer::get_from_path(path)?
        .ok_or_else(|| GlobalError::Other(format!("Unknown MIME type for: {:?}", path)))?
        .mime_type()
        .to_string();

    let base64 = STANDARD_NO_PAD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime_type, base64))
}

/// Encode an image file as an ImageUrl struct
///
/// # Arguments
/// * `path` - Path to the image file
///
/// # Returns
/// An ImageUrl struct suitable for use in ContentPart
pub fn encode_image_url(path: &Path) -> GlobalResult<ImageUrl> {
    let url = encode_image_data_url(path)?;
    Ok(ImageUrl { url, detail: None })
}

/// Build multi-modal message content with images
///
/// # Arguments
/// * `text` - The text content
/// * `image_paths` - Paths to image files
///
/// # Returns
/// MessageContent with both text and images
///
/// # Example
/// ```ignore
/// let content = build_vision_message(
///     "What's in this image?",
///     &["/path/to/image.png".to_string()]
/// )?;
/// ```
pub fn build_vision_message(text: &str, image_paths: &[String]) -> GlobalResult<MessageContent> {
    let mut parts = vec![ContentPart::Text {
        text: text.to_string(),
    }];

    for path_str in image_paths {
        let path = Path::new(path_str);
        let image_url = encode_image_url(path)?;
        parts.push(ContentPart::Image { image_url });
    }

    Ok(MessageContent::Parts(parts))
}

/// Build a ChatMessage with vision content
///
/// # Arguments
/// * `text` - The text content
/// * `image_paths` - Paths to image files
///
/// # Returns
/// A ChatMessage ready to send to an LLM
///
/// # Example
/// ```ignore
/// let msg = build_vision_chat_message(
///     "Describe this image",
///     &["/path/to/image.jpg".to_string()]
/// )?;
/// ```
pub fn build_vision_chat_message(text: &str, image_paths: &[String]) -> GlobalResult<ChatMessage> {
    let content = build_vision_message(text, image_paths)?;

    Ok(ChatMessage {
        role: Role::User,
        content: Some(content),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    })
}

/// Build a ChatMessage with a single image
///
/// # Arguments
/// * `text` - The text content
/// * `image_path` - Path to a single image file
///
/// # Returns
/// A ChatMessage with text and one image
pub fn build_vision_chat_message_single(text: &str, image_path: &str) -> GlobalResult<ChatMessage> {
    build_vision_chat_message(text, &[image_path.to_string()])
}

/// Create an ImageUrl from a URL string
///
/// # Arguments
/// * `url` - URL string (can be a web URL or data URL)
///
/// # Returns
/// An ImageUrl struct
pub fn image_url_from_string(url: impl Into<String>) -> ImageUrl {
    ImageUrl {
        url: url.into(),
        detail: None,
    }
}

/// Create an ImageUrl with detail level
///
/// # Arguments
/// * `url` - URL string
/// * `detail` - Detail level (low, high, auto)
///
/// # Returns
/// An ImageUrl struct with specified detail level
pub fn image_url_with_detail(url: impl Into<String>, detail: ImageDetail) -> ImageUrl {
    ImageUrl {
        url: url.into(),
        detail: Some(detail),
    }
}

/// Extension trait for ImageDetail with helper methods
pub trait ImageDetailExt {
    /// Convert to string for API
    fn as_str(&self) -> &str;
}

impl ImageDetailExt for ImageDetail {
    fn as_str(&self) -> &str {
        match self {
            ImageDetail::Low => "low",
            ImageDetail::High => "high",
            ImageDetail::Auto => "auto",
        }
    }
}

/// Check if a file is an image based on its extension
///
/// # Arguments
/// * `path` - Path to check
///
/// # Returns
/// true if the file appears to be an image
pub fn is_image_file(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(
            ext.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
        ),
        None => false,
    }
}

/// Get MIME type for a file path
///
/// # Arguments
/// * `path` - Path to the file
///
/// # Returns
/// The MIME type string or an error
pub fn get_mime_type(path: &Path) -> GlobalResult<String> {
    infer::get_from_path(path)?
        .ok_or_else(|| GlobalError::Other(format!("Unknown MIME type for: {:?}", path)))
        .map(|info| info.mime_type().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_image_file() {
        assert!(is_image_file(Path::new("test.png")));
        assert!(is_image_file(Path::new("test.JPG")));
        assert!(is_image_file(Path::new("test.jpeg")));
        assert!(!is_image_file(Path::new("test.txt")));
        assert!(!is_image_file(Path::new("test.pdf")));
    }

    #[test]
    fn test_image_detail_as_str() {
        assert_eq!(ImageDetail::Low.as_str(), "low");
        assert_eq!(ImageDetail::High.as_str(), "high");
        assert_eq!(ImageDetail::Auto.as_str(), "auto");
    }

    #[test]
    fn test_image_url_from_string() {
        let url = image_url_from_string("https://example.com/image.png");
        assert_eq!(url.url, "https://example.com/image.png");
        assert!(url.detail.is_none());
    }

    #[test]
    fn test_image_url_with_detail() {
        let url = image_url_with_detail("https://example.com/image.png", ImageDetail::High);
        assert_eq!(url.url, "https://example.com/image.png");
        assert!(url.detail.is_some());
    }
}
