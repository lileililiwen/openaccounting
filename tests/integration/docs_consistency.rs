//! Docs-consistency checks (`release-readiness`).
//!
//! These tests pin the repository hygiene gates without needing a
//! database: each check runs as a `python3 scripts/*.py` subprocess
//! against a `tempfile` fixture tree, asserting the exact failure
//! mode from the spec scenarios. Companion tests assert the checks
//! pass against the real checkout.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_check(script: &str, args: &[&str], cwd: &Path) -> (bool, String) {
    let out = Command::new("python3")
        .arg(repo_root().join("scripts").join(script))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("python3 must be available to run docs checks");
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), combined)
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

// ── 1.1: TBD markers ─────────────────────────────────────────────

#[test]
fn docs_lint_fails_on_tbd_marker_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let spec_dir = dir.path().join("specs").join("demo");
    write(
        &spec_dir.join("spec.md"),
        "# demo Specification\n\n## Purpose\nTBD - created by archiving change x. Update Purpose after archive.\n",
    );
    let arg = format!("--specs-dir={}", dir.path().join("specs").display());
    let (ok, out) = run_check("check_tbd_markers.py", &[arg.as_str()], dir.path());
    assert!(!ok, "TBD marker must fail the check; got:\n{out}");
    assert!(
        out.contains("TBD"),
        "failure must name the marker; got:\n{out}"
    );
}

#[test]
fn docs_lint_passes_without_tbd_markers() {
    let dir = tempfile::tempdir().unwrap();
    let spec_dir = dir.path().join("specs").join("demo");
    write(
        &spec_dir.join("spec.md"),
        "# demo Specification\n\n## Purpose\nReal contract text.\n",
    );
    let arg = format!("--specs-dir={}", dir.path().join("specs").display());
    let (ok, out) = run_check("check_tbd_markers.py", &[arg.as_str()], dir.path());
    assert!(ok, "clean specs must pass; got:\n{out}");
}

#[test]
fn docs_lint_ignores_marker_quoted_outside_purpose() {
    // Requirements may name the banned marker (e.g. the rule that
    // bans it); only a placeholder Purpose is a violation.
    let dir = tempfile::tempdir().unwrap();
    let spec_dir = dir.path().join("specs").join("demo");
    write(
        &spec_dir.join("spec.md"),
        "# demo Specification\n\n## Purpose\nReal contract text.\n\n## ADDED Requirements\n\n### Requirement: No placeholders\n\nNo spec SHALL contain `TBD - created by archiving`.\n\n#### Scenario: Marker found\n\n- **WHEN** any spec contains the marker\n- **THEN** the check fails.\n",
    );
    let arg = format!("--specs-dir={}", dir.path().join("specs").display());
    let (ok, out) = run_check("check_tbd_markers.py", &[arg.as_str()], dir.path());
    assert!(ok, "marker outside Purpose must pass; got:\n{out}");
}

#[test]
fn docs_lint_passes_on_real_specs() {
    let root = repo_root();
    let (ok, out) = run_check("check_tbd_markers.py", &[], root.as_path());
    assert!(
        ok,
        "real openspec/specs must carry no TBD markers; got:\n{out}"
    );
}

// ── 1.1: stale identity ──────────────────────────────────────────

#[test]
fn docs_lint_fails_on_stale_identity_fixture() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("README.md"), "# Fixture\n");
    write(
        &dir.path().join("docs").join("guide.md"),
        "Verify at github.com/anomalyco/other .\n",
    );
    let arg = format!("--root={}", dir.path().display());
    let (ok, out) = run_check("check_identity.py", &[arg.as_str()], dir.path());
    assert!(!ok, "stale identity must fail the check; got:\n{out}");
    assert!(
        out.contains("guide.md"),
        "failure must name the file; got:\n{out}"
    );
}

#[test]
fn docs_lint_passes_on_real_checkout_identity() {
    let root = repo_root();
    let (ok, out) = run_check("check_identity.py", &[], root.as_path());
    assert!(
        ok,
        "real checkout must carry no stale identity; got:\n{out}"
    );
}

// ── 1.1: broken doc paths ────────────────────────────────────────

#[test]
fn docs_lint_fails_on_broken_doc_path_fixture() {
    let dir = tempfile::tempdir().unwrap();
    write(
        &dir.path().join("README.md"),
        "# Fixture\n\nSee [missing](docs/no-such-file.md).\n",
    );
    let arg = format!("--root={}", dir.path().display());
    let (ok, out) = run_check("check_doc_paths.py", &[arg.as_str()], dir.path());
    assert!(!ok, "broken doc path must fail the check; got:\n{out}");
    assert!(
        out.contains("no-such-file.md"),
        "failure must name the path; got:\n{out}"
    );
}

#[test]
fn docs_lint_passes_on_real_checkout_paths() {
    let root = repo_root();
    let (ok, out) = run_check("check_doc_paths.py", &[], root.as_path());
    assert!(
        ok,
        "real docs must reference only existing paths; got:\n{out}"
    );
}

// ── 1.2: roadmap entries ─────────────────────────────────────────

#[test]
fn roadmap_check_flags_change_missing_from_roadmap() {
    let dir = tempfile::tempdir().unwrap();
    let changes = dir.path().join("changes");
    std::fs::create_dir_all(changes.join("some-new-thing")).unwrap();
    std::fs::create_dir_all(changes.join("archive")).unwrap();
    let roadmap = dir.path().join("ROADMAP.md");
    write(&roadmap, "# Roadmap\n\nOnly old work is listed.\n");
    let c_arg = format!("--changes-dir={}", changes.display());
    let r_arg = format!("--roadmap={}", roadmap.display());
    let (ok, out) = run_check(
        "check_roadmap_entries.py",
        &[c_arg.as_str(), r_arg.as_str()],
        dir.path(),
    );
    assert!(!ok, "unlisted change must fail the check; got:\n{out}");
    assert!(
        out.contains("some-new-thing"),
        "failure must name the change; got:\n{out}"
    );
}

#[test]
fn roadmap_check_passes_on_real_checkout() {
    let root = repo_root();
    let (ok, out) = run_check("check_roadmap_entries.py", &[], root.as_path());
    assert!(
        ok,
        "all active changes must be listed in ROADMAP.md; got:\n{out}"
    );
}

// ── Changelog discipline ─────────────────────────────────────────

#[test]
fn changelog_check_flags_missing_migration_notes() {
    let dir = tempfile::tempdir().unwrap();
    let migrations = dir.path().join("migrations");
    write(
        &migrations.join("0001_demo.sql"),
        "CREATE TABLE demo (id UUID);\n",
    );
    let changelog = dir.path().join("CHANGELOG.md");
    write(
        &changelog,
        "# Changelog\n\n## [Unreleased]\n\n### Added\n- Something.\n",
    );
    let c_arg = format!("--changelog={}", changelog.display());
    let m_arg = format!("--migrations-dir={}", migrations.display());
    let (ok, out) = run_check(
        "check_changelog.py",
        &[c_arg.as_str(), m_arg.as_str()],
        dir.path(),
    );
    assert!(!ok, "missing migration notes must fail; got:\n{out}");
    assert!(
        out.contains("migration"),
        "failure must name migration notes; got:\n{out}"
    );
}

#[test]
fn changelog_check_passes_on_real_checkout() {
    let root = repo_root();
    let (ok, out) = run_check("check_changelog.py", &[], root.as_path());
    assert!(
        ok,
        "real CHANGELOG must have Unreleased migration notes; got:\n{out}"
    );
}

// ── 1.3: ERD generation ──────────────────────────────────────────

const FIXTURE_MIGRATION: &str = "\
CREATE TABLE IF NOT EXISTS owners (\n\
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),\n\
    name TEXT NOT NULL\n\
);\n\
CREATE TABLE IF NOT EXISTS pets (\n\
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),\n\
    owner_id UUID NOT NULL REFERENCES owners(id) ON DELETE CASCADE,\n\
    name TEXT NOT NULL\n\
);\n";

#[test]
fn erd_generation_matches_checked_in_diagram() {
    let dir = tempfile::tempdir().unwrap();
    let migrations = dir.path().join("migrations");
    write(&migrations.join("0001_demo.sql"), FIXTURE_MIGRATION);
    let diagram = dir.path().join("erd.md");
    let m_arg = format!("--migrations-dir={}", migrations.display());
    let o_arg = format!("--output={}", diagram.display());

    // Generate the checked-in artifact from fixture migrations.
    let (ok, out) = run_check(
        "generate_erd.py",
        &[m_arg.as_str(), o_arg.as_str()],
        dir.path(),
    );
    assert!(ok, "generation must succeed; got:\n{out}");
    let text = std::fs::read_to_string(&diagram).unwrap();
    assert!(
        text.contains("owners"),
        "diagram must list owners; got:\n{text}"
    );
    assert!(
        text.contains("pets"),
        "diagram must list pets; got:\n{text}"
    );
    assert!(
        text.contains("owner_id"),
        "diagram must show the FK column; got:\n{text}"
    );

    // A matching checked-in copy passes --check ...
    let (ok, out) = run_check(
        "generate_erd.py",
        &[m_arg.as_str(), o_arg.as_str(), "--check"],
        dir.path(),
    );
    assert!(ok, "--check must pass on a fresh diagram; got:\n{out}");

    // ... and a stale copy fails --check.
    write(&diagram, "# stale diagram\n");
    let (ok, out) = run_check(
        "generate_erd.py",
        &[m_arg.as_str(), o_arg.as_str(), "--check"],
        dir.path(),
    );
    assert!(!ok, "--check must fail on a stale diagram; got:\n{out}");
}

#[test]
fn erd_check_passes_on_real_checkout() {
    let root = repo_root();
    let (ok, out) = run_check("generate_erd.py", &["--check"], root.as_path());
    assert!(
        ok,
        "checked-in docs/erd.md must match migrations; got:\n{out}"
    );
}
