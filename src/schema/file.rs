//! File metadata schema builder for validating file uploads and multipart payloads.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    context::ValidationContext,
    error::{RuleParams, ValidationError},
    messages::MessagesProvider,
    rule::RuleContext,
    schema::Schema,
};

/// Validated file metadata extracted from incoming multipart envelopes or JSON descriptors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Original filename supplied by the client.
    pub client_filename: String,
    /// Byte length of the file payload.
    pub size: usize,
    /// Detected or declared MIME type (e.g. `image/png`).
    pub mime_type: String,
    /// Lowercase file extension without leading dot (e.g. `png`), if present.
    pub extension: Option<String>,
}

/// Fluent schema builder for validating uploaded file metadata and constraints.
#[derive(Clone, Default)]
pub struct FileSchema {
    min_size: Option<usize>,
    max_size: Option<usize>,
    allowed_extensions: Vec<String>,
    allowed_mime_types: Vec<String>,
    bail_on_fail: bool,
    inline_message: Option<String>,
}

impl FileSchema {
    /// Creates a new `FileSchema`.
    #[must_use]
    pub fn new() -> Self {
        <Self as Default>::default()
    }

    /// Enforces a minimum file size in bytes.
    #[must_use]
    pub fn min_size(mut self, min: usize) -> Self {
        self.min_size = Some(min);
        self
    }

    /// Enforces a maximum file size in bytes.
    #[must_use]
    pub fn max_size(mut self, max: usize) -> Self {
        self.max_size = Some(max);
        self
    }

    /// Enforces an inclusive file size range in bytes `[min, max]`.
    #[must_use]
    pub fn size_between(self, min: usize, max: usize) -> Self {
        self.min_size(min).max_size(max)
    }

    /// Enforces an allowed whitelist of file extensions (case-insensitive, e.g. `["png", "jpg"]`).
    #[must_use]
    pub fn allowed_extensions(mut self, extensions: &[&str]) -> Self {
        self.allowed_extensions = extensions
            .iter()
            .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
            .collect();
        self
    }

    /// Enforces an allowed whitelist of MIME types (e.g. `["image/png", "application/pdf"]`).
    #[must_use]
    pub fn allowed_mime_types(mut self, mime_types: &[&str]) -> Self {
        self.allowed_mime_types = mime_types
            .iter()
            .map(|mime| mime.to_ascii_lowercase())
            .collect();
        self
    }

    /// Configures short-circuiting on failure.
    #[must_use]
    pub fn bail(mut self, enabled: bool) -> Self {
        self.bail_on_fail = enabled;
        self
    }

    /// Sets an inline custom message override.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.inline_message = Some(message.into());
        self
    }
}

impl Schema<FileMetadata> for FileSchema {
    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<FileMetadata, ValidationError> {
        let rule_ctx = RuleContext::new(path, context, parent_data, messages);
        let inline = self.inline_message.as_deref();

        let map = match value {
            Value::Object(m) => m,
            Value::Null => {
                let issue = rule_ctx.create_issue("required", RuleParams::new(), inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
            _ => {
                let issue = rule_ctx.create_issue("file", RuleParams::new(), inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let client_filename = map
            .get("client_filename")
            .or_else(|| map.get("filename"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let size = map
            .get("size")
            .and_then(Value::as_u64)
            .map(|s| s as usize)
            .unwrap_or(0);

        let mime_type = map
            .get("mime_type")
            .or_else(|| map.get("detected_mime_type"))
            .or_else(|| map.get("client_content_type"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let extension = map
            .get("extension")
            .and_then(Value::as_str)
            .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
            .or_else(|| {
                std::path::Path::new(&client_filename)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_ascii_lowercase())
            });

        let mut errors = ValidationError::new();

        if let Some(min) = self.min_size
            && size < min
        {
            let mut params = RuleParams::new();
            params.insert("min", min.to_string());
            params.insert("actual", size.to_string());
            errors.push(rule_ctx.create_issue("file_min_size", params, inline));
            if self.bail_on_fail {
                return Err(errors);
            }
        }

        if let Some(max) = self.max_size
            && size > max
        {
            let mut params = RuleParams::new();
            params.insert("max", max.to_string());
            params.insert("actual", size.to_string());
            errors.push(rule_ctx.create_issue("file_max_size", params, inline));
            if self.bail_on_fail {
                return Err(errors);
            }
        }

        if !self.allowed_extensions.is_empty() {
            let is_allowed = match &extension {
                Some(ext) => self.allowed_extensions.iter().any(|allowed| allowed == ext),
                None => false,
            };
            if !is_allowed {
                let mut params = RuleParams::new();
                params.insert("allowed", self.allowed_extensions.join(", "));
                errors.push(rule_ctx.create_issue("file_extension", params, inline));
                if self.bail_on_fail {
                    return Err(errors);
                }
            }
        }

        if !self.allowed_mime_types.is_empty() {
            let lower_mime = mime_type.to_ascii_lowercase();
            let is_allowed = self
                .allowed_mime_types
                .iter()
                .any(|allowed| allowed == &lower_mime);
            if !is_allowed {
                let mut params = RuleParams::new();
                params.insert("allowed", self.allowed_mime_types.join(", "));
                errors.push(rule_ctx.create_issue("file_mime_type", params, inline));
                if self.bail_on_fail {
                    return Err(errors);
                }
            }
        }

        if errors.is_empty() {
            Ok(FileMetadata {
                client_filename,
                size,
                mime_type,
                extension,
            })
        } else {
            Err(errors)
        }
    }

    fn to_json_schema_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("type".to_string(), Value::String("object".to_string()));
        let mut props = serde_json::Map::new();
        props.insert(
            "client_filename".to_string(),
            serde_json::json!({"type": "string"}),
        );
        props.insert(
            "size".to_string(),
            serde_json::json!({"type": "integer", "minimum": 0}),
        );
        props.insert(
            "mime_type".to_string(),
            serde_json::json!({"type": "string"}),
        );
        props.insert(
            "extension".to_string(),
            serde_json::json!({"type": ["string", "null"]}),
        );
        map.insert("properties".to_string(), Value::Object(props));
        map.insert(
            "required".to_string(),
            serde_json::json!(["client_filename", "size", "mime_type"]),
        );
        Value::Object(map)
    }
}
