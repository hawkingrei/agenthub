use std::env;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn styles_css_path() -> PathBuf {
    let repo_candidate = repo_root().join("web/src/styles.css");
    if repo_candidate.exists() {
        return repo_candidate;
    }

    if let Ok(test_srcdir) = env::var("TEST_SRCDIR") {
        let mut candidates = Vec::new();
        if let Ok(test_workspace) = env::var("TEST_WORKSPACE") {
            candidates.push(
                PathBuf::from(&test_srcdir)
                    .join(test_workspace)
                    .join("web/src/styles.css"),
            );
        }
        candidates.push(PathBuf::from(&test_srcdir).join("_main/web/src/styles.css"));
        candidates.push(PathBuf::from(&test_srcdir).join("agenthub/web/src/styles.css"));

        for path in candidates {
            if path.exists() {
                return path;
            }
        }
    }

    panic!("styles.css path not found in repo root or Bazel runfiles");
}

#[test]
fn styles_keep_runtime_shell_constraints() {
    let path = styles_css_path();
    let css = fs::read_to_string(&path).expect("styles.css should be readable");
    assert!(
        css.contains("--notion-font:"),
        "styles.css should define the shared sans-serif font stack"
    );
    assert!(
        css.contains("--notion-mono:"),
        "styles.css should define the shared monospace font stack"
    );
    assert!(
        !css.contains("fonts.googleapis.com"),
        "styles.css should not depend on remote font imports"
    );
    assert!(
        css.contains(":root {"),
        "styles.css should define root variables"
    );
    assert!(
        css.contains("--agenthub-vh: 100vh;"),
        "styles.css should define viewport height fallback variable"
    );
    assert!(
        css.contains("--agenthub-vw: 100vw;"),
        "styles.css should define viewport width fallback variable"
    );
    assert!(
        css.contains("--agenthub-header-height: 56px;"),
        "styles.css should define the runtime header height variable"
    );
    assert!(
        css.contains("@supports (height: 100dvh)"),
        "styles.css should keep dynamic viewport height support"
    );
    assert!(
        css.contains("html, body {"),
        "html/body styles should exist"
    );
    assert!(
        css.contains("width: 100%;"),
        "html/body should fill the viewport width"
    );
    assert!(
        css.contains("height: 100%;"),
        "html/body should fill the viewport height"
    );
    assert!(
        css.contains("overflow: hidden;"),
        "html/body should prevent outer-page scrolling"
    );
    assert!(css.contains(".app {"), "app shell styles should exist");
    assert!(
        css.contains("display: flex;"),
        "app shell should be flex-based"
    );
    assert!(
        css.contains("flex-direction: column;"),
        "app shell should stack header and workspace vertically"
    );
    assert!(
        css.contains("height: 100vh;"),
        "app shell should keep viewport-height fallback"
    );
    assert!(
        css.contains("height: 100dvh;"),
        "app shell should keep dynamic viewport-height support"
    );
    assert!(
        css.contains("::-webkit-scrollbar"),
        "styles.css should define custom scrollbar styling"
    );
    assert!(
        css.contains("font-family: var(--notion-mono);"),
        "styles.css should keep the shared monospace helper"
    );
    assert!(
        css.contains("button:not([class*=\"mantine-\"])"),
        "styles.css should keep non-Mantine button resets scoped"
    );
    assert!(
        css.contains("textarea:not([class*=\"mantine-\"])"),
        "styles.css should keep non-Mantine field resets scoped"
    );
    assert!(
        !css.contains(".acp-conversation"),
        "legacy ACP layout selectors should no longer be required in styles.css after Tailwind migration"
    );
}
