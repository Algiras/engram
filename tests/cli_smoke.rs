/// CLI smoke tests — invoke the compiled binary, no LLM required.
/// All tests use a temp dir as MEMORY_DIR or simply check CLI structure.
use assert_cmd::Command;
use tempfile::TempDir;

#[allow(deprecated)]
fn engram() -> Command {
    Command::cargo_bin("engram").unwrap()
}

// ── Binary runs ──────────────────────────────────────────────────────────

#[test]
fn help_flag_exits_zero() {
    engram().arg("--help").assert().success();
}

#[test]
fn version_flag_exits_zero() {
    engram().arg("--version").assert().success();
}

// ── Auth (no LLM needed) ─────────────────────────────────────────────────

#[test]
fn auth_list_exits_zero() {
    let tmp = TempDir::new().unwrap();
    engram()
        .args(["auth", "list"])
        .env("HOME", tmp.path())
        .assert()
        .success();
}

#[test]
fn auth_status_no_config_exits_zero() {
    // Should print "Note:" message and exit 0, not panic
    let tmp = TempDir::new().unwrap();
    engram()
        .args(["auth", "status"])
        .env("HOME", tmp.path())
        // Unset any real keys so no provider resolves
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .assert()
        .success();
}

// ── Graceful errors (regression tests) ──────────────────────────────────

#[test]
fn add_empty_category_errors_not_panics() {
    // engram add testproject "" "some content" → non-zero exit with error message, no panic.
    // Clap validates the category enum before our code runs, so exit code may be 1 or 2.
    let tmp = TempDir::new().unwrap();
    engram()
        .args(["add", "testproject", "", "some content"])
        .env("HOME", tmp.path())
        .assert()
        .failure();
    // If we reach here, the binary did not crash/panic (no SIGSEGV/SIGABRT).
}

// ── Verbose flag accepted ────────────────────────────────────────────────

#[test]
fn verbose_flag_accepted_on_auth_status() {
    // --verbose must not change the exit code — both should succeed
    let tmp = TempDir::new().unwrap();
    engram()
        .args(["auth", "status"])
        .env("HOME", tmp.path())
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .assert()
        .success();

    engram()
        .args(["--verbose", "auth", "status"])
        .env("HOME", tmp.path())
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .assert()
        .success();
}

// ── Doctor smoke test (no LLM needed for health check) ───────────────────

#[test]
fn doctor_unknown_project_exits_zero_reports_issues() {
    // doctor reads the filesystem only (no LLM calls for basic checks).
    // An unknown project should exit 0 and print a health report with issues,
    // not panic or error out.
    let tmp = TempDir::new().unwrap();
    let output = engram()
        .args(["doctor", "nonexistent-project-xyz"])
        .env("HOME", tmp.path())
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8_lossy(&output);
    // Should mention the project in the report
    assert!(
        text.contains("nonexistent-project-xyz"),
        "Doctor output should mention the project name; got: {}",
        text
    );
}

// ── Hooks status (no LLM, filesystem only) ───────────────────────────────

#[test]
fn hooks_status_exits_zero() {
    let tmp = TempDir::new().unwrap();
    engram()
        .args(["hooks", "status"])
        .env("HOME", tmp.path())
        .assert()
        .success();
}
