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
        css.contains(".acp {\n  height: 100%;\n  min-height: 0;\n  display: grid;\n  gap: 12px;\n  grid-template-columns: minmax(0, 1fr);\n  grid-template-rows: auto minmax(0, 1fr);\n  flex: 1;\n  overflow: hidden;\n}"),
        "acp container should be grid and height constrained"
    );
    assert!(
        css.contains(".acp-conversation {\n  overflow: auto;\n  min-height: 0;\n  height: 100%;\n  max-height: 100%;\n  display: block;\n}"),
        "acp conversation should be scrollable container"
    );
    assert!(
        css.contains(".acp-conversation-inner {\n  min-height: 100%;\n  display: flex;\n  flex-direction: column;\n  justify-content: flex-end;\n}"),
        "acp conversation inner should bottom-align short content"
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
        css.contains(".acp-bubble.agent_plan"),
        "acp plan bubble should be styled"
    );
    assert!(
        css.contains(".output-body {\n  max-height: none;\n  overflow: hidden;\n  display: flex;\n  flex-direction: column;\n  gap: 6px;\n  padding-right: 6px;\n  min-height: 0;\n  flex: 1;\n}"),
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
        css.contains(".input.docked {\n  background: #fff;\n  border: 1px solid #e0e0e0;\n  border-radius: 12px;\n  padding: 10px;\n  box-shadow: var(--shadow);\n  margin-top: auto;\n  position: relative;\n  display: grid;\n  gap: 8px;\n  grid-template-rows: auto auto;\n  align-items: stretch;\n}"),
        "input docked should use grid slots for actions and editor rows"
    );
    assert!(
        css.contains(".input-row {\n  position: static;\n  display: flex;\n  align-items: center;\n  justify-content: flex-start;\n  flex-wrap: wrap;\n  gap: 6px;\n  min-height: 28px;\n}"),
        "input actions row should be flow layout and left aligned for consistent chip placement"
    );
    assert!(
        css.contains(".input-editor-row {\n  display: grid;\n  grid-template-columns: minmax(0, 1fr) auto;\n  align-items: stretch;\n  gap: 12px;\n  min-width: 0;\n}"),
        "input editor row should isolate textarea and send button layout"
    );
    assert!(
        css.contains(".input-editor-row .input-send-button {\n  min-height: 48px !important;\n  min-width: 92px;\n  padding: 0 16px !important;\n  border-radius: 10px !important;\n  font-size: 14px !important;\n  line-height: 1.1;\n  align-self: stretch;\n}"),
        "send button should keep larger tap target size"
    );
    assert!(
        css.contains("@supports (height: 100dvh)"),
        "styles.css should use dynamic viewport height fallback for mobile browsers"
    );
    assert!(
        css.contains(".admin .card li:not([class*=\"mantine-\"])"),
        "admin list layout styles should be scoped and not override markdown lists"
    );
    assert!(
        css.contains(".acp-head.minimal {\n    flex-direction: column;\n    align-items: stretch;\n    gap: 6px;\n  }"),
        "mobile ACP head should stack controls for narrow screens"
    );
    assert!(
        css.contains("--acp-tab-font-size: clamp(10px, 2.8vw, 11px);"),
        "mobile ACP tabs should use adaptive font sizing"
    );
    assert!(
        css.contains(".acp-tabs .tab {\n    display: inline-flex;\n    align-items: center;\n    justify-content: center;\n    flex: 1 1 0;\n    min-height: 30px;"),
        "mobile ACP top tabs should be adaptive and centered"
    );
    assert!(
        css.contains(".acp-debug-tabs {\n    max-width: 100%;\n    flex-wrap: wrap;"),
        "mobile ACP debug tabs should wrap adaptively on narrow screens"
    );
}
