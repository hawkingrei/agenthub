import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";
import { WorkbenchHeaderMenu } from "./components/workbench_header_menu";
import { mantineTheme } from "./ui/mantine_theme";

function renderMenu(props: Partial<React.ComponentProps<typeof WorkbenchHeaderMenu>> = {}) {
  return renderToStaticMarkup(
    <MantineProvider theme={mantineTheme}>
      <WorkbenchHeaderMenu
        active="agents"
        username="root"
        isRoot={true}
        onLogout={() => {}}
        onNavigate={() => {}}
        buttonClassName="menu-button"
        defaultOpened={true}
        {...props}
      />
    </MantineProvider>
  );
}

describe("WorkbenchHeaderMenu", () => {
  it("renders workspace and account actions in one menu", () => {
    const html = renderMenu();
    expect(html).toContain("Agents");
    expect(html).toContain("Teams");
    expect(html).toContain("Settings");
    expect(html).toContain("Logout");
    expect(html).toContain("Menu");
  });

  it("hides settings for non-root users", () => {
    const html = renderMenu({ isRoot: false });
    expect(html).toContain("Agents");
    expect(html).toContain("Teams");
    expect(html).not.toContain("Settings");
    expect(html).toContain("Logout");
  });
});
