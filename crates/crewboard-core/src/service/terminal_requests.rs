//! Run requests (RUN-01, RUN-02, RUN-04): start, stop, status, the list of every Run that is up, and Ask Claude;
//! the Shell (RUN-05).

use super::Shared;
use super::board_requests::error_body;
use crate::protocol::{ErrorBody, ErrorKind, Reply, Request};

pub(super) async fn handle(request: Request, shared: &Shared) -> Result<Reply, ErrorBody> {
    let runs = &shared.runs;
    let run = |r: Result<crate::terminals::run::RunInfo, crate::board::BoardError>| {
        r.map(|info| Reply::Run(Box::new(info)))
            .map_err(|e| error_body(&e))
    };
    match request {
        Request::RunStart(p) => run(runs.start(&p.key).await),
        Request::RunStop(p) => run(runs.stop(&p.key).await),
        Request::RunStatus(p) => run(runs.info(&p.key).await),
        Request::RunList => Ok(Reply::Runs(runs.running().await)),
        Request::RunSuggest(p) => shared
            .suggester
            .start(&p.project)
            .await
            .map(|()| Reply::Ok)
            .map_err(|e| error_body(&e)),
        Request::ShellOpen(p) => shared
            .shells
            .open_shell(&p.key, p.reopen)
            .await
            .map(|s| Reply::Session(Box::new(s)))
            .map_err(|e| error_body(&e)),
        Request::RunSuggestCancel(p) => {
            shared.suggester.cancel(&p.project);
            Ok(Reply::Ok)
        }
        other => Err(ErrorBody::new(
            ErrorKind::BadRequest,
            format!("{} isn't a terminal request", other.method()),
        )),
    }
}
