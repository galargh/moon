mod utils;

use moon_affected::{Affected, AffectedProjectState, AffectedTaskState};
use moon_common::is_ci;
use moon_common::path::WorkspaceRelativePathBuf;
use moon_task::Target;
use rustc_hash::FxHashSet;
use starbase_utils::json::serde_json;
use utils::{change_branch, change_files, create_query_sandbox};

mod query_affected {
    use super::*;

    #[test]
    fn nothing_by_default() {
        let sandbox = create_query_sandbox();

        change_branch(&sandbox, "branch");

        sandbox
            .run_bin(|cmd| {
                cmd.arg("query").arg("affected");
            })
            .success()
            .stdout("{}\n");
    }

    #[test]
    fn includes_project_for_file() {
        let sandbox = create_query_sandbox();

        change_files(&sandbox, ["basic/file.txt"]);

        let assert = sandbox.run_bin(|cmd| {
            cmd.arg("query").arg("affected");
        });

        let mut affected: Affected = serde_json::from_str(assert.stdout().trim()).unwrap();

        assert!(!affected.projects.contains_key("advanced"));
        assert_eq!(
            affected.projects.remove("basic").unwrap(),
            AffectedProjectState {
                files: FxHashSet::from_iter(["basic/file.txt".into()]),
                tasks: FxHashSet::from_iter([Target::parse("basic:dev").unwrap()]),
                ..Default::default()
            }
        );
    }

    #[test]
    fn includes_task_for_input() {
        let sandbox = create_query_sandbox();

        change_files(&sandbox, ["tasks/tests/file.txt"]);

        let assert = sandbox.run_bin(|cmd| {
            cmd.arg("query").arg("affected");
        });

        let mut affected: Affected = serde_json::from_str(assert.stdout().trim()).unwrap();

        assert_eq!(
            affected
                .tasks
                .remove(&Target::parse("tasks:test").unwrap())
                .unwrap(),
            AffectedTaskState {
                files: FxHashSet::from_iter(["tasks/tests/file.txt".into()]),
                ..Default::default()
            }
        );
    }

    #[test]
    fn status_filters_out_non_matching() {
        let sandbox = create_query_sandbox();

        change_files(&sandbox, ["basic/file.txt"]);

        sandbox
            .run_bin(|cmd| {
                cmd.arg("query")
                    .arg("affected")
                    .args(["--status", "deleted"]);
            })
            .success()
            .stdout("{}\n");
    }

    #[test]
    fn status_includes_matching() {
        let sandbox = create_query_sandbox();

        change_files(&sandbox, ["basic/file.txt"]);

        let status = if is_ci() { "added" } else { "untracked" };

        let assert = sandbox.run_bin(|cmd| {
            cmd.arg("query").arg("affected").args(["--status", status]);
        });

        let affected: Affected = serde_json::from_str(assert.stdout().trim()).unwrap();

        assert!(affected.projects.contains_key("basic"));
    }

    #[test]
    fn status_supports_multiple_values() {
        let sandbox = create_query_sandbox();

        change_files(&sandbox, ["basic/file.txt"]);

        let assert = sandbox.run_bin(|cmd| {
            cmd.arg("query")
                .arg("affected")
                .args(["--status", "added", "--status", "untracked"]);
        });

        let affected: Affected = serde_json::from_str(assert.stdout().trim()).unwrap();

        assert!(affected.projects.contains_key("basic"));
    }

    #[test]
    fn ci_excludes_tasks_disabled_in_ci() {
        let sandbox = create_query_sandbox();

        // basic:dev has preset: 'server' which sets run_in_ci: false
        change_files(&sandbox, ["basic/file.txt"]);

        let assert = sandbox.run_bin(|cmd| {
            cmd.arg("query").arg("affected").arg("--ci");
        });

        let affected: Affected = serde_json::from_str(assert.stdout().trim()).unwrap();

        let project = affected.projects.get("basic").unwrap();
        assert!(
            project
                .files
                .contains(&WorkspaceRelativePathBuf::from("basic/file.txt"))
        );
        assert!(!project.tasks.contains(&Target::parse("basic:dev").unwrap()));
    }

    #[test]
    fn without_ci_includes_all_tasks() {
        let sandbox = create_query_sandbox();

        change_files(&sandbox, ["basic/file.txt"]);

        let assert = sandbox.run_bin(|cmd| {
            cmd.arg("query").arg("affected").arg("--ci").arg("false");
        });

        let affected: Affected = serde_json::from_str(assert.stdout().trim()).unwrap();

        let project = affected.projects.get("basic").unwrap();
        assert!(project.tasks.contains(&Target::parse("basic:dev").unwrap()));
    }

    #[test]
    fn base_and_head_flags_accepted() {
        let sandbox = create_query_sandbox();

        change_branch(&sandbox, "branch");

        sandbox
            .run_bin(|cmd| {
                cmd.arg("query")
                    .arg("affected")
                    .args(["--base", "master", "--head", "branch"]);
            })
            .success()
            .stdout("{}\n");
    }

    #[test]
    fn stdin_can_be_disabled() {
        let sandbox = create_query_sandbox();

        change_files(&sandbox, ["basic/file.txt"]);

        let assert = sandbox.run_bin(|cmd| {
            cmd.arg("query").arg("affected").args(["--stdin", "false"]);
        });

        let affected: Affected = serde_json::from_str(assert.stdout().trim()).unwrap();

        assert!(affected.projects.contains_key("basic"));
    }

    #[test]
    fn include_relations_by_default_includes_upstream() {
        let sandbox = create_query_sandbox();

        change_files(&sandbox, ["basic/file.txt"]);

        let assert = sandbox.run_bin(|cmd| {
            cmd.arg("query")
                .arg("affected")
                .args(["--upstream", "deep"]);
        });

        let affected: Affected = serde_json::from_str(assert.stdout().trim()).unwrap();

        assert!(affected.projects.contains_key("no-config"));
    }

    #[test]
    fn without_include_relations_excludes_upstream_only() {
        let sandbox = create_query_sandbox();

        change_files(&sandbox, ["basic/file.txt"]);

        let assert = sandbox.run_bin(|cmd| {
            cmd.arg("query").arg("affected").args([
                "--upstream",
                "deep",
                "--include-relations",
                "false",
            ]);
        });

        let affected: Affected = serde_json::from_str(assert.stdout().trim()).unwrap();

        assert!(!affected.projects.contains_key("no-config"));
    }
}
