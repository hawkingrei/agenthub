import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import {
  TeamDebugToolsHeader,
  TeamRunOpsPanel,
  TeamRunRequiredPanel,
} from "./team_debug_panels";

const chrome = {
  panelCardClassName: "panel",
  sectionHintTextClassName: "hint",
  debugTabsClassName: "debug-tabs",
  debugTabActiveClassName: "debug-tab-active",
  debugTabIdleClassName: "debug-tab-idle",
  panelSecondaryButtonClassName: "secondary",
};

describe("Team debug panels", () => {
  it("renders the debug tools header with the active tab highlighted", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamDebugToolsHeader
          chrome={chrome}
          teamDebugTag="mailbox_raw"
          onTeamDebugTagChange={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Debug Tools");
    expect(html).toContain("Run Ops");
    expect(html).toContain("Step Ops");
    expect(html).toContain("Mailbox Raw");
    expect(html).toContain("debug-tab-active");
  });

  it("renders run ops controls and helper text", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamRunOpsPanel
          chrome={chrome}
          busy={null}
          runContextId="ctx-1"
          runInput='{"task":"sync"}'
          runLookupId="run-1"
          canCreateRun={true}
          runInputHasError={false}
          runInputError={null}
          createRunTitle="Create run"
          parsedRunInput={{ task: "sync" }}
          helperText="Accepts any valid JSON value."
          onRunContextIdChange={vi.fn()}
          onRunInputChange={vi.fn()}
          onRunLookupIdChange={vi.fn()}
          onCreateRun={vi.fn()}
          onLoadRunById={vi.fn()}
          onUseExampleJson={vi.fn()}
          onSetEmptyObject={vi.fn()}
          onFormatJson={vi.fn()}
          onClearRunInput={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Create Run");
    expect(html).toContain("Load Existing Run");
    expect(html).toContain("Use Example JSON");
    expect(html).toContain("Accepts any valid JSON value.");
  });

  it("renders the run-required empty state", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamRunRequiredPanel
          chrome={chrome}
          title="Mailbox Raw"
          body="Mailbox raw operations require an active execution run."
          onGoToRuns={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Mailbox Raw");
    expect(html).toContain("Mailbox raw operations require an active execution run.");
    expect(html).toContain("Go to Execution Runs");
  });
});
