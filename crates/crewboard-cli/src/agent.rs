//! The commands an agent uses to hand back control: `crewboard ask human --stdin` (ATT-01, d0037) and
//! `crewboard review submit [KEY] --stdin` (REV-07, d0050). Both take their text on stdin (PLG-05) and work on
//! `$CREWBOARD_TICKET` unless a key is given. `crewboard review feedback [KEY]` (REV-05, d0017) prints what the
//! latest Send back carried; it only reads.
//!
//! `ask human` takes 2–4 answer options (`--option "Postmark" --option "SES" --recommended 1`, d0061); the
//! service checks them.
//!
//! # `--json` shapes
//!
//! Both print the ticket afterwards (`Ticket`); after `ask human` its `question` holds the question's `id`,
//! `text`, `options` and `recommended` (1-based).
//!
//! `review feedback --json` prints `{"key", "round", "note"?, "comments": [{"file", "line", "side", "body"}], "pr"?}`;
//! `pr` (a ticket with a PR) is `{"number", "comments": [{"file"?, "line"?, "author", "text"}], "note"?}`, the
//! unresolved PR comments fetched while the command runs (T-0148)
//! (`round` 0 and no comments when the ticket was never sent back; `side` is `old` for a removed line).

use std::io::Read;

use crewboard_core::data_dir::DataDir;
use crewboard_core::naming;
use crewboard_core::protocol::{
    AnswerParams, AskHumanParams, AskTicketParams, ErrorBody, ErrorKind, KeyParams, Reply, Request,
    ReviewSubmitParams,
};

use crate::cli::GlobalArgs;
use crate::output::Output;
use crate::read::unexpected;
use crate::service::{self, client_error, internal};
use crate::target;

fn usage(message: impl Into<String>) -> ErrorBody {
    ErrorBody::new(ErrorKind::Usage, message)
}

fn read_stdin() -> Result<String, ErrorBody> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| internal(format!("can't read stdin: {e}")))?;
    Ok(text.trim().to_owned())
}

pub struct AskArgs {
    pub target: String,
    pub stdin: bool,
    pub options: Vec<String>,
    pub recommended: Option<u32>,
}

pub fn ask(args: AskArgs, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let cmd = naming::COMMAND_NAME;
    let AskArgs {
        target: target_name,
        stdin,
        options,
        recommended,
    } = args;
    let to_human = target_name.eq_ignore_ascii_case("human");
    if !to_human && (!options.is_empty() || recommended.is_some()) {
        return Err(usage("Options are only for questions to the human"));
    }
    if !stdin {
        return Err(usage(format!(
            "give the question on stdin: `{cmd} ask {target_name} --stdin`"
        )));
    }
    if !to_human {
        return ask_ticket(&target_name, global, out);
    }
    let question = read_stdin()?;
    if question.is_empty() {
        return Err(usage("the question on stdin is empty"));
    }
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, None, global).await?;
        let ticket = match client
            .request(Request::TicketAskHuman(AskHumanParams {
                key: key.to_string(),
                question,
                options,
                recommended,
            }))
            .await
            .map_err(client_error)?
        {
            Reply::Ticket(t) => *t,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &format!(
                "{} asks you. The question is on the board; end your turn and wait for the answer.",
                ticket.key
            ),
            &ticket,
            Some(&ticket.key.to_string()),
        );
        Ok(())
    })
}

/// `crewboard ask <KEY> --stdin` (MSG-01): a question to a linked ticket's agent; returns at once with its ID.
fn ask_ticket(target: &str, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let question = read_stdin()?;
    if question.is_empty() {
        return Err(usage("the question on stdin is empty"));
    }
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, None, global).await?;
        let target = target::resolve_key(&client, &data, Some(target.to_owned()), global).await?;
        let message = match client
            .request(Request::TicketAsk(AskTicketParams {
                key: key.to_string(),
                target: target.to_string(),
                question,
            }))
            .await
            .map_err(client_error)?
        {
            Reply::Message(m) => *m,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &format!(
                "Asked {} (question {}). Carry on: the answer is typed into your session when it arrives.",
                message.to, message.id
            ),
            &message,
            Some(&message.id),
        );
        Ok(())
    })
}

/// `crewboard answer <ID> --stdin`.
pub fn answer(id: String, stdin: bool, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let cmd = naming::COMMAND_NAME;
    if !stdin {
        return Err(usage(format!(
            "give the answer on stdin: `{cmd} answer {id} --stdin`"
        )));
    }
    let answer = read_stdin()?;
    if answer.is_empty() {
        return Err(usage("the answer on stdin is empty"));
    }
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let message = match client
            .request(Request::MessageAnswer(AnswerParams { id, answer }))
            .await
            .map_err(client_error)?
        {
            Reply::Message(m) => *m,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &format!(
                "Answered {}'s question {}. It is typed into {}'s session when its agent is idle.",
                message.from, message.id, message.from
            ),
            &message,
            Some(&message.id),
        );
        Ok(())
    })
}

/// `crewboard questions [KEY]`: the questions waiting for this ticket's answer.
pub fn questions(key: Option<String>, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let cmd = naming::COMMAND_NAME;
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, key, global).await?;
        let messages = match client
            .request(Request::TicketQuestions(KeyParams {
                key: key.to_string(),
            }))
            .await
            .map_err(client_error)?
        {
            Reply::Messages(m) => m,
            other => return Err(unexpected(&other)),
        };
        let text = if messages.is_empty() {
            format!("No questions wait for {key}.")
        } else {
            messages
                .iter()
                .map(|m| {
                    format!(
                        "{} · from {}\n  {}\n  answer: `{cmd} answer {} --stdin`",
                        m.id,
                        m.from,
                        m.question.replace('\n', "\n  "),
                        m.id
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        out.print(&text, &messages, None);
        Ok(())
    })
}

pub fn submit(
    key: Option<String>,
    stdin: bool,
    global: &GlobalArgs,
    out: &Output,
) -> Result<(), ErrorBody> {
    let summary = if stdin { read_stdin()? } else { String::new() };
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, key, global).await?;
        let ticket = match client
            .request(Request::TicketReviewSubmit(ReviewSubmitParams {
                key: key.to_string(),
                summary,
            }))
            .await
            .map_err(client_error)?
        {
            Reply::Ticket(t) => *t,
            other => return Err(unexpected(&other)),
        };
        // Validation on: the validator checks it first, and its result comes back to this session (VAL-01).
        let validating = matches!(
            ticket.status,
            crewboard_core::domain::Status::Validating
                | crewboard_core::domain::Status::WaitingForLimitReset
        );
        let text = if validating {
            crewboard_core::validation::SUBMITTED_VALIDATING.to_owned()
        } else {
            format!("{} is Ready for Review.", ticket.key)
        };
        out.print(&text, &ticket, Some(&ticket.key.to_string()));
        Ok(())
    })
}

pub fn feedback(key: Option<String>, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, key, global).await?;
        let feedback = match client
            .request(Request::ReviewFeedback(KeyParams {
                key: key.to_string(),
            }))
            .await
            .map_err(client_error)?
        {
            Reply::ReviewFeedback(f) => f,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &render_feedback(&feedback),
            &feedback,
            Some(&feedback.key.to_string()),
        );
        Ok(())
    })
}

/// The unresolved PR comments, after the in-app ones (REV-05):
///
/// ```text
/// PR #481 · unresolved comments
///
/// lib/totals.ts
///   12 (anna): Round per line, not per order.
/// ```
fn render_pr_comments(pr: &crewboard_core::pr::comments::PrFeedback) -> String {
    let mut out = format!("\n\nPR #{} · unresolved comments\n", pr.number);
    if let Some(note) = &pr.note {
        out.push_str(&format!("\n{note}\n"));
        return out;
    }
    if pr.comments.is_empty() {
        out.push_str("\nNone.\n");
        return out;
    }
    let mut file: Option<Option<&str>> = None;
    for c in &pr.comments {
        if file != Some(c.file.as_deref()) {
            out.push_str(&format!(
                "\n{}\n",
                c.file.as_deref().unwrap_or("(the whole PR)")
            ));
            file = Some(c.file.as_deref());
        }
        let at = c.line.map(|l| format!("{l} ")).unwrap_or_default();
        let mut lines = c.text.lines();
        out.push_str(&format!(
            "  {at}({}): {}\n",
            c.author,
            lines.next().unwrap_or_default()
        ));
        for more in lines {
            out.push_str(&format!("    {more}\n"));
        }
    }
    out
}

/// ```text
/// SHOP-27 · feedback from round 2
///
/// Note: Also check page 0.
///
/// lib/search.ts
///   12: Use the page size from settings.
///   40 (removed line): Why was this removed?
/// ```
pub fn render_feedback(f: &crewboard_core::board::comments::ReviewFeedback) -> String {
    let mut out = render_review_feedback(f);
    if let Some(pr) = &f.pr {
        out.push_str(&render_pr_comments(pr));
    }
    out.trim_end().to_owned()
}

/// The in-app part: the latest Send back's note and line comments.
fn render_review_feedback(f: &crewboard_core::board::comments::ReviewFeedback) -> String {
    use crewboard_core::board::comments::Side;
    if f.round == 0 {
        return format!("{} has no review feedback: it was never sent back.", f.key);
    }
    let mut out = format!("{} · feedback from round {}\n", f.key, f.round);
    if let Some(note) = &f.note {
        out.push_str(&format!("\nNote: {note}\n"));
    }
    if f.comments.is_empty() {
        out.push_str("\nNo line comments.\n");
    }
    let mut file: Option<&str> = None;
    for c in &f.comments {
        if file != Some(c.file.as_str()) {
            out.push_str(&format!("\n{}\n", c.file));
            file = Some(&c.file);
        }
        let side = if c.side == Side::Old {
            " (removed line)"
        } else {
            ""
        };
        let mut lines = c.body.lines();
        out.push_str(&format!(
            "  {}{side}: {}\n",
            c.line,
            lines.next().unwrap_or_default()
        ));
        for more in lines {
            out.push_str(&format!("    {more}\n"));
        }
    }
    out.trim_end().to_owned()
}

/// `crewboard validate pass|fail --stdin` (VAL-03, CLI-05): the validator's verdict and report on its ticket
/// (`$CREWBOARD_TICKET`). Only a validator session may report one.
pub fn validate(
    command: crate::cli::ValidateCommand,
    global: &GlobalArgs,
    out: &Output,
) -> Result<(), ErrorBody> {
    let (pass, stdin) = match command {
        crate::cli::ValidateCommand::Pass { stdin } => (true, stdin),
        crate::cli::ValidateCommand::Fail { stdin } => (false, stdin),
    };
    let report = if stdin { read_stdin()? } else { String::new() };
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let key = target::resolve_key(&client, &data, None, global).await?;
        let ticket = match client
            .request(Request::ValidationVerdict(
                crewboard_core::protocol::VerdictParams {
                    key: key.to_string(),
                    pass,
                    report,
                },
            ))
            .await
            .map_err(client_error)?
        {
            Reply::Ticket(t) => *t,
            other => return Err(unexpected(&other)),
        };
        let text = if pass {
            format!("{}: validation passed. End your turn.", ticket.key)
        } else {
            format!("{}: validation failed. End your turn.", ticket.key)
        };
        out.print(&text, &ticket, Some(&ticket.key.to_string()));
        Ok(())
    })
}

/// `crewboard agent name "<name>" [--rename]` (PAG-04): a Project agent names itself; refused anywhere else, a
/// second time without `--rename`, and over the user's own name.
pub fn name(name: &str, rename: bool, global: &GlobalArgs, out: &Output) -> Result<(), ErrorBody> {
    let data = DataDir::from_env().map_err(|e| internal(e.to_string()))?;
    service::runtime()?.block_on(async {
        let client = service::connect(&data, global).await?;
        let agent = match client
            .request(Request::AgentName(
                crewboard_core::protocol::AgentNameParams {
                    name: name.to_owned(),
                    rename,
                },
            ))
            .await
            .map_err(client_error)?
        {
            Reply::ProjectAgent(a) => *a,
            other => return Err(unexpected(&other)),
        };
        out.print(
            &format!("Named \"{}\".", agent.name),
            &agent,
            Some(&agent.id),
        );
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crewboard_core::board::comments::ReviewFeedback;
    use crewboard_core::pr::comments::{PrComment, PrFeedback};

    fn feedback(pr: Option<PrFeedback>) -> ReviewFeedback {
        ReviewFeedback {
            key: "SHOP-26".parse().unwrap(),
            round: 0,
            note: None,
            comments: Vec::new(),
            pr,
        }
    }

    /// REV-05: the unresolved PR comments come after the in-app feedback, by file.
    #[test]
    fn pr_comments_follow_the_in_app_feedback() {
        let text = render_feedback(&feedback(Some(PrFeedback {
            number: 481,
            comments: vec![
                PrComment {
                    file: Some("lib/totals.ts".into()),
                    line: Some(12),
                    author: "anna".into(),
                    text: "Round per line.\nNot per order.".into(),
                },
                PrComment {
                    file: None,
                    line: None,
                    author: "bo".into(),
                    text: "Add a changelog line.".into(),
                },
            ],
            note: None,
        })));
        assert_eq!(
            text,
            "SHOP-26 has no review feedback: it was never sent back.\n\nPR #481 · unresolved comments\n\n\
             lib/totals.ts\n  12 (anna): Round per line.\n    Not per order.\n\n(the whole PR)\n  (bo): Add a \
             changelog line."
        );
        let text = render_feedback(&feedback(Some(PrFeedback {
            number: 481,
            comments: Vec::new(),
            note: Some(
                "PR #481 comments weren't fetched: `gh` isn't logged in (`gh auth login`)".into(),
            ),
        })));
        assert!(
            text.ends_with("`gh` isn't logged in (`gh auth login`)"),
            "{text}"
        );
        assert!(!render_feedback(&feedback(None)).contains("PR #"));
    }
}
