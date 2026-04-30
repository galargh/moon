use crate::app_options::AffectedOption;
use crate::queries::changed_files::*;
use crate::session::MoonSession;
use clap::Args;
use moon_affected::{AffectedTracker, DownstreamScope, UpstreamScope};
use moon_common::is_ci;
use moon_vcs::ChangedStatus;
use starbase::AppResult;
use starbase_utils::json;
use tracing::instrument;

#[derive(Args, Clone, Debug)]
pub struct QueryAffectedArgs {
    #[arg(help = "Conditions in which to track affected")]
    by: Option<AffectedOption>,

    #[arg(
        long,
        help = "Evaluate the pipeline as if it's a CI environment",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    ci: Option<bool>,

    #[arg(long, help = "Filter files based on a changed status")]
    status: Vec<ChangedStatus>,

    #[arg(
        long,
        default_value_t,
        visible_alias = "dependents",
        help = "Include downstream dependents"
    )]
    downstream: DownstreamScope,

    #[arg(
        long,
        default_value_t,
        visible_alias = "dependencies",
        help = "Include upstream dependencies"
    )]
    upstream: UpstreamScope,
}

#[instrument(skip(session))]
pub async fn affected(session: MoonSession, args: QueryAffectedArgs) -> AppResult {
    let vcs = session.get_vcs_adapter()?;
    let ci = is_ci();

    let mut options = QueryChangedFilesOptions {
        default_branch: ci,
        local: !ci,
        stdin: true,
        ..Default::default()
    };

    if let Some(by) = &args.by {
        options.apply_affected(by);
    }

    if !args.status.is_empty() {
        options.status = args.status;
    }

    let changed_files = query_changed_files(&vcs, options)
        .await
        .map(|result| result.files)?;

    let mut affected_tracker =
        AffectedTracker::new(session.get_workspace_graph().await?, changed_files);
    // Defaults to false (not is_ci()) for backwards compatibility:
    // this command previously had no CI filtering in any environment.
    affected_tracker.set_ci_check(args.ci.unwrap_or(false));
    affected_tracker.set_scopes(args.upstream, args.downstream);

    if session.workspace_config.experiments.async_affected_tracking {
        affected_tracker.track_projects_async().await?;
        affected_tracker.track_tasks_async().await?;
    } else {
        affected_tracker.track_projects()?;
        affected_tracker.track_tasks()?;
    }

    let affected = affected_tracker.build();

    session
        .console
        .out
        .write_line(json::format(&affected, true)?)?;

    Ok(None)
}
