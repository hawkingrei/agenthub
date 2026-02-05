use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn styles_keep_acp_conversation_scoped() {
    let path = repo_root().join("web/src/styles.css");
    let css = fs::read_to_string(&path).expect("styles.css should be readable");
    assert!(
        css.contains(".acp-conversation"),
        "styles.css should define .acp-conversation"
    );
    assert!(
        css.contains(".acp {\n  height: 100%;\n  min-height: 0;\n  display: flex;\n  flex-direction: column;\n  flex: 1;\n}"),
        "acp container should be flex and height constrained"
    );
    assert!(
        css.contains(".acp-conversation {\n  overflow: auto;\n  min-height: 0;\n  height: 100%;\n  max-height: 100%;\n  flex: 1 1 auto;\n  display: flex;\n  flex-direction: column;\n}"),
        "acp conversation should be scrollable and flex column"
    );
    assert!(
        css.contains(".acp-plan {\n  overflow: auto;\n  min-height: 0;\n  max-height: 100%;\n}"),
        "acp plan should be scrollable"
    );
    assert!(
        css.contains(".acp-thought-fold pre {\n  margin: 0;\n  white-space: pre-wrap;\n}"),
        "acp thought pre should preserve formatting"
    );
    assert!(
        css.contains(".acp-bubble.agent_thinking"),
        "acp thinking bubble should be styled"
    );
    assert!(
        css.contains(".output-body {\n  max-height: none;\n  overflow: hidden;\n  display: flex;\n  flex-direction: column;\n  gap: 12px;\n  padding-right: 6px;\n  min-height: 0;\n  flex: 1;\n}"),
        "output body should be height constrained and non-scrollable"
    );
    assert!(
        css.contains(".terminal {\n  font-family: \"Source Code Pro\", monospace;\n  background: #0e1116;\n  color: #c7d0df;\n  padding: 12px;\n  border-radius: 8px;\n  min-height: 220px;\n  overflow: auto;\n  white-space: pre-wrap;\n  min-height: 0;\n  height: 100%;\n}"),
        "terminal output should be scrollable inside output body"
    );
    assert!(
        css.contains("body {\n  margin: 0;\n  min-height: 100vh;\n  height: 100vh;\n  display: flex;\n  flex-direction: column;\n  overflow: hidden;\n}"),
        "body should be fixed height with overflow hidden"
    );
    assert!(
        css.contains(".app {\n  width: 100%;\n  margin: 0;\n  padding: 2px 8px 0;\n  min-height: 100vh;\n  height: 100vh;\n  overflow: auto;\n  display: flex;\n  flex-direction: column;\n  flex: 1;\n}"),
        "app should be fixed height with overflow auto"
    );
    assert!(
        css.contains(".input.docked {\n  background: #fff;\n  border: 1px solid #e0e0e0;\n  border-radius: 12px;\n  padding: 10px;\n  box-shadow: var(--shadow);\n  margin-top: auto;\n}"),
        "input docked should stick to bottom"
    );
}
