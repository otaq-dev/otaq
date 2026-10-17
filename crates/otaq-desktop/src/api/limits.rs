//! `limits.*`: a tool's usage limit (USG-10, AGT-08, d0086). `limits.resume_tool { tool }` is Resume on a tool's
//! banner line, its inbox row and a tool problem's notice (the pane's, the lead's): that tool's waiting agents
//! continue, with the same toast everywhere. `limits.move_waiting { tool, agent }` is Move waiting to… on the tool's
//! banner line: its not-yet-started tickets move to `agent` and start ("4 tickets moved to Strong GPT").

use otaq_core::protocol::ToolParams;
use serde_json::Value;

use super::{RequestError, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    let parse_error = |e: serde_json::Error| RequestError::plain(e.to_string());
    match method {
        "limits.resume_tool" => {
            let p: ToolParams = serde_json::from_value(params).map_err(parse_error)?;
            super::notices::resume_tool(app, p.tool).await
        }
        "limits.move_waiting" => {
            super::notices::move_waiting(app, serde_json::from_value(params).map_err(parse_error)?)
                .await
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

    #[tokio::test(flavor = "multi_thread")]
    async fn move_waiting_needs_a_tool_and_an_agent() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        assert!(
            request(&h.app, "limits.move_waiting", json!({ "tool": "codex" }))
                .await
                .is_err(),
            "no agent"
        );
        // An unknown agent: the service refuses, and the page gets an error toast, not an error.
        request(
            &h.app,
            "limits.move_waiting",
            json!({ "tool": "claude", "agent": "nobody" }),
        )
        .await
        .unwrap();
        let view = request(&h.app, "notices.view", serde_json::Value::Null)
            .await
            .unwrap();
        assert_eq!(view["toasts"][0]["title"], "Move waiting failed");
        h.service.stop_all().await;
    }
}
