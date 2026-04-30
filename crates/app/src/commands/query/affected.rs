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
    #[arg(env = "MOON_AFFECTED", help = "Conditions in which to track affected")]
    by: Option<AffectedOption>,

    #[arg(
        long,
        help = "Evaluate the pipeline as if it's a CI environment",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    ci: Option<bool>,

    #[arg(
        long,
        env = "MOON_BASE",
        help = "Base branch, commit, or revision to compare against"
    )]
    base: Option<String>,

    #[arg(
        long,
        env = "MOON_HEAD",
        help = "Current branch, commit, or revision to compare with"
    )]
    head: Option<String>,

    #[arg(
        short = 'g',
        long,
        env = "MOON_INCLUDE_RELATIONS",
        help = "Include graph relations for affected checks, instead of just changed files",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    // Defaults to true, unlike exec-based commands where it defaults to false.
    include_relations: Option<bool>,

    #[arg(long, help = "Filter files based on a changed status")]
    status: Vec<ChangedStatus>,

    #[arg(
        long,
        help = "Accept changed files from stdin for affected checks",
        default_missing_value = "true",
        num_args = 0..=1
    )]
    stdin: Option<bool>,

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
        // This should default to false (matching moon exec), but that would
        // be a breaking change since this command defaulted to true before
        // the --stdin option was introduced.
        stdin: args.stdin.unwrap_or(true),
        ..Default::default()
    };

    if let Some(by) = &args.by {
        options.apply_affected(by);
    }

    if args.base.is_some() {
        options.base = args.base;
    }

    if args.head.is_some() {
        options.head = args.head;
    }

    if !args.status.is_empty() {
        options.status = args.status;
    }

    let changed_files = query_changed_files(&vcs, options)
        .await
        .map(|result| result.files)?;

    let mut affected_tracker =
        AffectedTracker::new(session.get_workspace_graph().await?, changed_files);
    // This should default to is_ci(), but that would be a breaking change
    // since this command defaulted to false in all environments before
    // the --ci option was introduced.
    affected_tracker.set_ci_check(args.ci.unwrap_or(false));
    affected_tracker.set_scopes(args.upstream, args.downstream);

    if session.workspace_config.experiments.async_affected_tracking {
        affected_tracker.track_projects_async().await?;
        affected_tracker.track_tasks_async().await?;
    } else {
        affected_tracker.track_projects()?;
        affected_tracker.track_tasks()?;
    }

    let mut affected = affected_tracker.build();

    if !args.include_relations.unwrap_or(true) {
        affected
            .projects
            .retain(|_, state| !state.files.is_empty() || state.other);
        affected
            .tasks
            .retain(|_, state| !state.files.is_empty() || !state.env.is_empty() || state.other);
    }

    session
        .console
        .out
        .write_line(json::format(&affected, true)?)?;

    Ok(None)
}
