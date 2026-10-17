//! `limits.*`: a tool's usage limit (USG-10, d0086). `limits.resume_tool { tool }` is Resume after reset in a tool
//! problem's notice (the pane's, the lead's): that tool's waiting agents continue, with the same toast as the
//! inbox's limit row.

use otaq_core::protocol::ToolParams;
use serde_json::Value;

use super::{RequestError, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "limits.resume_tool" => {
            let p: ToolParams =
                serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))?;
            super::notices::resume_tool(app, p.tool).await
        }
        _ => Err(unknown(method)),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::api::request;
    use crate::service::testing::{Harness, connected};

    #[tokio::test(flavor = "multi_thread")]
    async fn resume_tool_takes_a_tool_and_refuses_anything_else() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        request(&h.app, "limits.resume_tool", json!({ "tool": "codex" }))
            .await
            .unwrap();
        assert!(
            request(&h.app, "limits.resume_tool", json!({ "tool": "nope" }))
                .await
                .is_err()
        );
    }
}
