use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn mctl_status_stdouts(workspace: &Path) -> Vec<String> {
    let mut stdouts = Vec::new();

    for args in [vec!["status"], vec!["status", "--detailed"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_mctl"))
            .current_dir(workspace)
            .args(&args)
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();

        assert!(output.status.success(), "{stdout}");
        stdouts.push(stdout);
    }

    stdouts
}

fn assert_synced(workspace: &Path, synced: bool) {
    for stdout in mctl_status_stdouts(workspace) {
        assert_eq!(
            stdout.contains("All repositories are synced"),
            synced,
            "{stdout}"
        );
    }
}

/// Creates a workspace with one repository pinned by `pin` (a TOML line such
/// as `tag = "v1"`), checked out on a detached HEAD at its first commit.
fn workspace_with_pinned_repository(
    root: &Path,
    pin: impl Fn(&str) -> String,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let workspace = root.join("workspace");
    let pinned = workspace.join("pinned");
    fs::create_dir_all(&pinned).unwrap();

    git(&pinned, &["init", "-b", "main"]);
    git(&pinned, &["config", "user.email", "test@example.com"]);
    git(&pinned, &["config", "user.name", "Test"]);
    git(&pinned, &["commit", "--allow-empty", "-m", "pinned"]);
    git(&pinned, &["tag", "-a", "v1", "-m", "v1"]);
    let pinned_commit = git(&pinned, &["rev-parse", "HEAD"]);
    git(&pinned, &["commit", "--allow-empty", "-m", "later"]);
    git(&pinned, &["checkout", "--detach", &pinned_commit]);

    fs::write(
        workspace.join("mirror.toml"),
        format!(
            "[[repositories]]\ngit = \"git@example.com:org/pinned.git\"\npath = \"pinned\"\n{}\n",
            pin(&pinned_commit)
        ),
    )
    .unwrap();

    (workspace, pinned)
}

#[test]
fn repository_pinned_by_tag_is_synced_only_while_head_is_the_pinned_commit() {
    let temp = TempDir::new().unwrap();
    let (workspace, pinned) =
        workspace_with_pinned_repository(temp.path(), |_| "tag = \"v1\"".to_owned());

    assert_synced(&workspace, true);

    fs::write(pinned.join("local.txt"), "change").unwrap();
    assert_synced(&workspace, false);
    fs::remove_file(pinned.join("local.txt")).unwrap();

    git(&pinned, &["checkout", "--detach", "main"]);
    assert_synced(&workspace, false);
}

#[test]
fn repository_pinned_by_rev_is_synced_on_the_pinned_commit() {
    let temp = TempDir::new().unwrap();
    let (workspace, _pinned) =
        workspace_with_pinned_repository(temp.path(), |commit| format!("rev = \"{commit}\""));

    assert_synced(&workspace, true);
}

#[test]
fn detached_repository_without_a_pin_needs_attention() {
    let temp = TempDir::new().unwrap();
    let (workspace, _pinned) = workspace_with_pinned_repository(temp.path(), |_| String::new());

    assert_synced(&workspace, false);
}
