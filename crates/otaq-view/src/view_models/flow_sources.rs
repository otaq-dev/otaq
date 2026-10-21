//! The go-ahead's line naming what the agent looks at (ONB-04, ONB-08; d0108 smaller rule 6; *s14-12*, *s14-18*),
//! built from what there is ([`Sources`], ONB-03): "Claude looks at how you use Claude Code on this Mac (what you
//! typed in it, your skills and CLAUDE.md files), this repo and GitHub, and proposes a flow for shop-web." Only
//! what is there is named; the agent is the Project agent's tool. The Add project dialog's short form drops the
//! parenthesis. Nothing at all gives "Nothing to look at yet" and its reason, and the presets instead. Both the Flow
//! step and the Add project dialog read this view.

use otaq_core::agent_tools::Tool;
use otaq_core::onboarding;
use otaq_core::tools::Os;
use serde::Serialize;
use ts_rs::TS;

/// Whether the go-ahead offers **Look and propose a flow**, or only the presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "FlowLookKind")]
pub enum Kind {
    Look,
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowLook")]
pub struct View {
    pub kind: Kind,
    /// The Flow step's line (*s14-12*); with nothing, "Nothing to look at yet · pick a preset to start. …".
    pub line: String,
    /// The Add project dialog's line, without the parenthesis (*s14-18*); with nothing, the same as `line`.
    pub short_line: String,
    /// Why there is nothing, under the line; only with [`Kind::Nothing`].
    pub reason: Option<String>,
}

pub const NOTHING: &str = "Nothing to look at yet \u{b7} pick a preset to start. You can change it later by asking the Project agent.";

/// What the line names: M34's sources. Since M35 (T-0433) Otaq checks no sources, so [`view`] builds this from the
/// connected tools only ("this repo", looked at by the first one); T-0434 replaces this view with ASK-05's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    pub project_name: String,
    pub history: bool,
    pub commits: bool,
    pub gh: bool,
    pub glab: bool,
    pub tool: Tool,
}

impl Sources {
    fn any(&self) -> bool {
        self.history || self.commits || self.gh || self.glab
    }
}

/// The view for who can be asked, on this computer.
pub fn view(sources: &onboarding::Sources) -> View {
    let first = sources.tools.first().copied();
    view_on(
        Os::current(),
        &Sources {
            project_name: sources.project_name.clone(),
            history: false,
            commits: first.is_some(),
            gh: false,
            glab: false,
            tool: first.unwrap_or(Tool::ALL[0]),
        },
    )
}

/// The view as `os` words it: "this Mac" on macOS, else "this computer".
pub fn view_on(os: Os, sources: &Sources) -> View {
    let computer = match os {
        Os::Mac => "this Mac",
        Os::Windows | Os::Linux => "this computer",
    };
    if !sources.any() {
        return View {
            kind: Kind::Nothing,
            line: NOTHING.to_owned(),
            short_line: NOTHING.to_owned(),
            reason: Some(format!(
                "No Claude Code history on {computer}, no commits in this repo yet, and neither gh nor glab is \
                 signed in."
            )),
        };
    }
    let line = |history: String| {
        let mut parts = Vec::new();
        if sources.history {
            parts.push(history);
        }
        if sources.commits {
            parts.push("this repo".to_owned());
        }
        if sources.gh {
            parts.push("GitHub".to_owned());
        }
        if sources.glab {
            parts.push("GitLab".to_owned());
        }
        // "A, B and C, and proposes…"; one alone takes no comma ("this repo and proposes…").
        let comma = if parts.len() > 1 { "," } else { "" };
        format!(
            "{} looks at {}{comma} and proposes a flow for {}.",
            sources.tool.short_name(),
            and_list(&parts),
            sources.project_name
        )
    };
    let history = format!("how you use Claude Code on {computer}");
    View {
        kind: Kind::Look,
        line: line(format!(
            "{history} (what you typed in it, your skills and CLAUDE.md files)"
        )),
        short_line: line(history),
        reason: None,
    }
}

/// "A", "A and B", "A, B and C".
fn and_list(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources(history: bool, commits: bool, gh: bool, glab: bool) -> Sources {
        Sources {
            project_name: "shop-web".into(),
            history,
            commits,
            gh,
            glab,
            tool: Tool::Claude,
        }
    }

    fn line(s: Sources) -> String {
        view_on(Os::Mac, &s).line
    }

    #[test]
    fn the_frames_lines() {
        // s14-12: history, a repo with commits, gh.
        let shop = view_on(Os::Mac, &sources(true, true, true, false));
        assert_eq!(shop.kind, Kind::Look);
        assert_eq!(
            shop.line,
            "Claude looks at how you use Claude Code on this Mac (what you typed in it, your skills and CLAUDE.md \
             files), this repo and GitHub, and proposes a flow for shop-web."
        );
        assert_eq!(shop.reason, None);
        // s14-18: history, a repo, glab; the dialog's short form.
        let billing = view_on(
            Os::Mac,
            &Sources {
                project_name: "billing-api".into(),
                ..sources(true, true, false, true)
            },
        );
        assert_eq!(
            billing.short_line,
            "Claude looks at how you use Claude Code on this Mac, this repo and GitLab, and proposes a flow for \
             billing-api."
        );
    }

    #[test]
    fn only_what_is_there_is_named() {
        assert_eq!(
            line(sources(true, false, false, false)),
            "Claude looks at how you use Claude Code on this Mac (what you typed in it, your skills and CLAUDE.md \
             files) and proposes a flow for shop-web."
        );
        assert_eq!(
            view_on(Os::Mac, &sources(true, false, false, false)).short_line,
            "Claude looks at how you use Claude Code on this Mac and proposes a flow for shop-web."
        );
        assert_eq!(
            line(sources(false, true, false, false)),
            "Claude looks at this repo and proposes a flow for shop-web."
        );
        assert_eq!(
            line(sources(false, false, true, false)),
            "Claude looks at GitHub and proposes a flow for shop-web."
        );
        assert_eq!(
            line(sources(false, false, false, true)),
            "Claude looks at GitLab and proposes a flow for shop-web."
        );
        assert_eq!(
            line(sources(false, true, true, false)),
            "Claude looks at this repo and GitHub, and proposes a flow for shop-web."
        );
        assert_eq!(
            line(sources(false, false, true, true)),
            "Claude looks at GitHub and GitLab, and proposes a flow for shop-web."
        );
        assert_eq!(
            view_on(Os::Mac, &sources(true, true, true, true)).short_line,
            "Claude looks at how you use Claude Code on this Mac, this repo, GitHub and GitLab, and proposes a flow \
             for shop-web."
        );
    }

    #[test]
    fn every_combination_names_each_fact_it_has() {
        for n in 1..16u8 {
            let s = sources(n & 1 != 0, n & 2 != 0, n & 4 != 0, n & 8 != 0);
            let v = view_on(Os::Mac, &s);
            assert_eq!(v.kind, Kind::Look);
            assert_eq!(v.reason, None);
            assert!(v.line.starts_with("Claude looks at "), "{}", v.line);
            assert!(v.line.ends_with(" proposes a flow for shop-web."));
            assert_eq!(
                v.line.contains("Claude Code on this Mac (what you typed"),
                s.history
            );
            assert_eq!(v.short_line.contains("Claude Code on this Mac"), s.history);
            assert!(!v.short_line.contains('('));
            assert_eq!(v.line.contains("this repo"), s.commits);
            assert_eq!(v.line.contains("GitHub"), s.gh);
            assert_eq!(v.line.contains("GitLab"), s.glab);
        }
    }

    #[test]
    fn nothing_shows_the_presets_and_why() {
        let v = view_on(Os::Mac, &sources(false, false, false, false));
        assert_eq!(v.kind, Kind::Nothing);
        assert_eq!(
            v.line,
            "Nothing to look at yet \u{b7} pick a preset to start. You can change it later by asking the Project agent."
        );
        assert_eq!(v.short_line, v.line);
        assert_eq!(
            v.reason.as_deref(),
            Some(
                "No Claude Code history on this Mac, no commits in this repo yet, and neither gh nor glab is signed in."
            )
        );
        assert_eq!(
            view_on(Os::Linux, &sources(false, false, false, false))
                .reason
                .as_deref(),
            Some(
                "No Claude Code history on this computer, no commits in this repo yet, and neither gh nor glab is \
                 signed in."
            )
        );
    }

    #[test]
    fn the_agent_is_the_project_agents_tool() {
        for (tool, name) in [
            (Tool::Claude, "Claude"),
            (Tool::Codex, "Codex"),
            (Tool::Copilot, "Copilot"),
            (Tool::OpenCode, "OpenCode"),
        ] {
            let v = view_on(
                Os::Mac,
                &Sources {
                    tool,
                    ..sources(false, true, false, false)
                },
            );
            assert_eq!(
                v.line,
                format!("{name} looks at this repo and proposes a flow for shop-web.")
            );
        }
        assert_eq!(
            view_on(Os::Windows, &sources(true, false, false, false)).short_line,
            "Claude looks at how you use Claude Code on this computer and proposes a flow for shop-web."
        );
    }
}
