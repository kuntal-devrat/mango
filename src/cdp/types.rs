//! Chrome DevTools Protocol (CDP) Data Types & Protocol Structures.
//!
//! Conforming to the Chrome DevTools Protocol 1.3 specification for
//! automated headless driving via Puppeteer, Playwright, and DevTools clients.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Information describing an active browser target (page, iframe, worker).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub target_id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub title: String,
    pub url: String,
    pub attached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_context_id: Option<String>,
}

/// JSON payload returned by HTTP GET `/json/version`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct VersionInfo {
    pub browser: String,
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: String,
    #[serde(rename = "User-Agent")]
    pub user_agent: String,
    #[serde(rename = "V8-Version")]
    pub v8_version: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: String,
}

/// Target summary returned in HTTP GET `/json/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetDescriptor {
    pub id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub title: String,
    pub url: String,
    pub description: String,
    pub web_socket_debugger_url: String,
    pub devtools_frontend_url: String,
}

/// Incoming JSON-RPC 2.0 command payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

/// Outgoing JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

/// Standard JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value, session_id: Option<String>) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
            session_id,
        }
    }

    pub fn error(
        id: Option<Value>,
        code: i64,
        message: impl Into<String>,
        session_id: Option<String>,
    ) -> Self {
        Self {
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            session_id,
        }
    }

    pub fn method_not_found(id: Option<Value>, method: &str, session_id: Option<String>) -> Self {
        Self::error(
            id,
            -32601,
            format!("Method not found: {method}"),
            session_id,
        )
    }

    pub fn invalid_params(id: Option<Value>, message: &str, session_id: Option<String>) -> Self {
        Self::error(id, -32602, message, session_id)
    }
}

/// Outgoing CDP event/notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub method: String,
    pub params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

/// Remote JavaScript object representation in CDP Runtime domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteObject {
    #[serde(rename = "type")]
    pub object_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
}

impl RemoteObject {
    pub fn string(s: impl Into<String>) -> Self {
        let val = s.into();
        Self {
            object_type: "string".to_string(),
            subtype: None,
            class_name: None,
            description: Some(val.clone()),
            value: Some(Value::String(val)),
            object_id: None,
        }
    }

    pub fn number(n: f64) -> Self {
        Self {
            object_type: "number".to_string(),
            subtype: None,
            class_name: None,
            description: Some(n.to_string()),
            value: serde_json::Number::from_f64(n).map(Value::Number),
            object_id: None,
        }
    }

    pub fn boolean(b: bool) -> Self {
        Self {
            object_type: "boolean".to_string(),
            subtype: None,
            class_name: None,
            description: Some(b.to_string()),
            value: Some(Value::Bool(b)),
            object_id: None,
        }
    }

    pub fn null() -> Self {
        Self {
            object_type: "object".to_string(),
            subtype: Some("null".to_string()),
            class_name: None,
            description: Some("null".to_string()),
            value: Some(Value::Null),
            object_id: None,
        }
    }

    pub fn undefined() -> Self {
        Self {
            object_type: "undefined".to_string(),
            subtype: None,
            class_name: None,
            description: Some("undefined".to_string()),
            value: None,
            object_id: None,
        }
    }
}

/// Box model geometry quad arrays returned by `DOM.getBoxModel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxModel {
    pub content: [f32; 8],
    pub padding: [f32; 8],
    pub border: [f32; 8],
    pub margin: [f32; 8],
    pub width: f32,
    pub height: f32,
}

/// Single computed style name-value pair returned by `CSS.getComputedStyleForNode`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputedStyleProperty {
    pub name: String,
    pub value: String,
}

/// Cookie record returned by `Network.getCookies`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdpCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: f64,
    pub size: usize,
    pub http_only: bool,
    pub secure: bool,
    pub session: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
}
