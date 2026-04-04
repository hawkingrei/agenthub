import { expect, test } from "./coverage";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const stylesPath = fileURLToPath(new URL("../../src/styles.css", import.meta.url));
const styles = readFileSync(stylesPath, "utf8");
const dockFixtureStyles = `
  .app {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    overflow: hidden;
  }

  .workspace,
  .workspace.collapsed {
    display: flex;
    min-height: 0;
    width: 100%;
    flex: 1 1 auto;
    overflow: hidden;
  }

  .workspace-left,
  .workspace-left.collapsed {
    display: flex;
    min-height: 0;
    width: 56px;
    flex: 0 0 56px;
  }

  .workspace-right {
    display: flex;
    min-height: 0;
    flex: 1 1 auto;
    flex-direction: column;
    overflow: hidden;
  }

  .output-body {
    min-height: 0;
    flex: 1 1 auto;
    overflow: auto;
  }

  .input.docked {
    position: sticky;
    bottom: 20px;
    left: 50%;
    display: flex;
    width: min(calc(100vw - 24px), 760px);
    max-width: 100%;
    flex-direction: column;
    gap: 6px;
    margin: auto auto 0;
    padding: 10px;
    border: 1px solid rgba(15, 23, 42, 0.08);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.92);
    box-shadow: 0 10px 30px rgba(15, 23, 42, 0.1);
    backdrop-filter: blur(12px);
  }

  .input-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .input-editor-row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }

  .input-history {
    position: relative;
  }

  .history-toggle,
  .input-interrupt-button,
  .input-send-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 600;
  }

  .history-toggle,
  .input-interrupt-button {
    min-height: 32px;
    padding: 0 12px;
    border: 1px solid rgba(15, 23, 42, 0.08);
    background: #fff;
  }

  .input-send-button {
    min-height: 44px;
    padding: 0 14px;
    border: none;
    background: #2383e2;
    color: #fff;
  }

  .input-history-menu {
    position: absolute;
    bottom: calc(100% + 0.5rem);
    left: 0;
    z-index: 30;
    min-width: 15rem;
    max-width: min(26rem, calc(100vw - 48px));
    max-height: 15rem;
    overflow-y: auto;
    border: 1px solid rgba(15, 23, 42, 0.08);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.98);
    padding: 6px;
    box-shadow: 0 20px 24px rgba(25, 25, 25, 0.05);
  }

  .input-history-item {
    display: block;
    width: 100%;
    padding: 8px 12px;
    text-align: left;
  }

  textarea {
    flex: 1 1 auto;
    min-height: 44px;
    padding: 6px 4px;
    border: none;
    background: transparent;
    resize: none;
  }
`;

async function mountInputDock(page: import("@playwright/test").Page): Promise<void> {
  await page.setContent(`
    <style>${styles}\n${dockFixtureStyles}</style>
    <main style="padding: 8px;">
      <div class="input docked" style="max-width: 920px;">
        <div class="input-row" role="group" aria-label="Input actions">
          <button class="acp-interrupt-button input-interrupt-button">Interrupt</button>
          <div class="input-history">
            <button class="history-toggle" aria-expanded="true">History</button>
            <div class="input-history-menu">
              <button class="input-history-item">git status</button>
              <button class="input-history-item">cargo test</button>
            </div>
          </div>
        </div>
        <div class="input-editor-row">
          <textarea rows="2">echo hello</textarea>
          <button class="input-send-button">Send</button>
        </div>
      </div>
    </main>
  `);
}

async function mountWorkspaceWithDock(
  page: import("@playwright/test").Page,
  options: { collapsed?: boolean } = {}
): Promise<void> {
  const collapsed = options.collapsed ?? false;
  const workspaceClass = collapsed ? "workspace collapsed" : "workspace";
  const workspaceLeftClass = collapsed ? "workspace-left collapsed" : "workspace-left";
  await page.setContent(`
    <style>${styles}\n${dockFixtureStyles}</style>
    <section class="${workspaceClass}" style="height: var(--agenthub-vh, 760px);">
      <div class="${workspaceLeftClass}">
        <div class="agent-layout">
          <h2>Agents</h2>
        </div>
      </div>
      <div class="workspace-right" style="height: 100%;">
        <div class="output-header"><h2>Output</h2></div>
        <div class="output-body" style="overflow: auto;">
          <div style="height: 1200px; background: linear-gradient(#f6f8fb, #e8edf6);"></div>
        </div>
        <div class="input docked">
          <div class="input-row" role="group" aria-label="Input actions"></div>
          <div class="input-editor-row">
            <textarea rows="2">echo hello</textarea>
            <button class="input-send-button">Send</button>
          </div>
        </div>
      </div>
    </section>
  `);
}

async function mountAppShellWithDock(
  page: import("@playwright/test").Page,
  options: { collapsed?: boolean } = {}
): Promise<void> {
  const collapsed = options.collapsed ?? true;
  const workspaceClass = collapsed ? "workspace collapsed" : "workspace";
  const workspaceLeftClass = collapsed ? "workspace-left collapsed" : "workspace-left";
  await page.setContent(`
    <style>${styles}\n${dockFixtureStyles}</style>
    <div class="app">
      <section class="${workspaceClass}" style="height: 100%;">
        <div class="${workspaceLeftClass}">
          <div class="agent-layout">
            <h2>Agents</h2>
          </div>
        </div>
        <div class="workspace-right" style="height: 100%;">
          <div class="output-header"><h2>Output</h2></div>
          <div class="output-body" style="overflow: auto;">
            <div style="height: 1200px; background: linear-gradient(#f6f8fb, #e8edf6);"></div>
          </div>
          <div class="input docked">
            <div class="input-row" role="group" aria-label="Input actions"></div>
            <div class="input-editor-row">
              <textarea rows="2">echo hello</textarea>
              <button class="input-send-button">Send</button>
            </div>
          </div>
        </div>
      </section>
    </div>
  `);
}

async function assertDockAnchoredToWorkspaceBottom(
  page: import("@playwright/test").Page,
  tolerance = 6
): Promise<void> {
  const workspaceBox = await page.locator(".workspace-right").boundingBox();
  const dockBox = await page.locator(".input.docked").boundingBox();
  expect(workspaceBox).not.toBeNull();
  expect(dockBox).not.toBeNull();
  const workspace = workspaceBox!;
  const dock = dockBox!;
  expect(dock.y + dock.height).toBeGreaterThanOrEqual(
    workspace.y + workspace.height - tolerance
  );
}

async function assertDockLayout(
  page: import("@playwright/test").Page,
  expectedChipMinHeight: number,
  expectedSendMinHeight: number
): Promise<void> {
  const rowBox = await page.locator(".input-row").boundingBox();
  const editorRowBox = await page.locator(".input-editor-row").boundingBox();
  const interruptBox = await page.locator(".input-interrupt-button").boundingBox();
  const historyBox = await page.locator(".history-toggle").boundingBox();
  const menuBox = await page.locator(".input-history-menu").boundingBox();
  const sendBox = await page.locator(".input-send-button").boundingBox();
  const textareaBox = await page.locator("textarea").boundingBox();

  expect(rowBox).not.toBeNull();
  expect(editorRowBox).not.toBeNull();
  expect(interruptBox).not.toBeNull();
  expect(historyBox).not.toBeNull();
  expect(menuBox).not.toBeNull();
  expect(sendBox).not.toBeNull();
  expect(textareaBox).not.toBeNull();

  const row = rowBox!;
  const editor = editorRowBox!;
  const interrupt = interruptBox!;
  const history = historyBox!;
  const menu = menuBox!;
  const send = sendBox!;
  const textarea = textareaBox!;

  // Action row should stay above editor row and never overlap textarea.
  expect(row.y + row.height).toBeLessThanOrEqual(editor.y + 0.5);
  expect(row.y + row.height).toBeLessThanOrEqual(textarea.y + 0.5);

  // Left-aligned chips keep a stable anchor at the start of the row.
  expect(interrupt.x).toBeGreaterThanOrEqual(row.x - 0.5);
  expect(interrupt.x).toBeLessThanOrEqual(row.x + 2.5);
  expect(history.x).toBeGreaterThanOrEqual(interrupt.x + interrupt.width - 1);

  // History popup should be left-anchored to the History trigger.
  expect(menu.x).toBeGreaterThanOrEqual(history.x - 1);
  expect(menu.x).toBeLessThanOrEqual(history.x + 2);

  // Tap target sizes for touch ergonomics.
  expect(interrupt.height).toBeGreaterThanOrEqual(expectedChipMinHeight);
  expect(history.height).toBeGreaterThanOrEqual(expectedChipMinHeight);
  expect(send.height).toBeGreaterThanOrEqual(expectedSendMinHeight);
}

test("keeps input dock controls touch-friendly on mobile viewport", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mountInputDock(page);
  await assertDockLayout(page, 24, 40);
});

test("keeps input dock controls non-overlapping on tablet viewports", async ({
  page,
}) => {
  for (const viewport of [
    { width: 820, height: 1180 },
    { width: 1180, height: 820 },
  ]) {
    await page.setViewportSize(viewport);
    await mountInputDock(page);
    await assertDockLayout(page, 26, 44);
  }
});

test("keeps docked input anchored to the workspace bottom on mobile viewport", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mountWorkspaceWithDock(page, { collapsed: true });
  await assertDockAnchoredToWorkspaceBottom(page);
});

test("keeps docked input anchored while agents panel toggles collapsed/expanded on mobile", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mountWorkspaceWithDock(page, { collapsed: false });
  await assertDockAnchoredToWorkspaceBottom(page);

  await page.evaluate(() => {
    document.querySelector(".workspace")?.classList.add("collapsed");
    document.querySelector(".workspace-left")?.classList.add("collapsed");
  });
  await assertDockAnchoredToWorkspaceBottom(page);

  await page.evaluate(() => {
    document.querySelector(".workspace")?.classList.remove("collapsed");
    document.querySelector(".workspace-left")?.classList.remove("collapsed");
  });
  await assertDockAnchoredToWorkspaceBottom(page);
});

test("keeps docked input anchored through simulated keyboard open/close viewport transitions", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mountWorkspaceWithDock(page, { collapsed: true });
  await page.evaluate(() => {
    document.documentElement.style.setProperty("--agenthub-vh", "844px");
  });
  await assertDockAnchoredToWorkspaceBottom(page);

  await page.evaluate(() => {
    document.documentElement.style.setProperty("--agenthub-vh", "620px");
  });
  await page.setViewportSize({ width: 390, height: 620 });
  await assertDockAnchoredToWorkspaceBottom(page);

  await page.evaluate(() => {
    document.documentElement.style.setProperty("--agenthub-vh", "844px");
  });
  await page.setViewportSize({ width: 390, height: 844 });
  await assertDockAnchoredToWorkspaceBottom(page);
});

test("keeps docked input flush with viewport bottom in app shell on mobile", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mountAppShellWithDock(page, { collapsed: true });
  const dockBox = await page.locator(".input.docked").boundingBox();
  expect(dockBox).not.toBeNull();
  const dock = dockBox!;
  expect(dock.y + dock.height).toBeGreaterThanOrEqual(844 - 3);
});
