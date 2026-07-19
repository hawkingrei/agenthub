import { describe, expect, it } from "vitest";

const TEAM_ROUTE_SYMBOLS = [
  "buildCanonicalTeamWorkspaceSubpath",
  "buildTeamDetailPath",
  "buildTeamWorkspacePath",
  "resolveTeamMemberRouteTab",
  "resolveTeamRoute",
  "resolveTeamWorkspacePathState",
  "resolveWorkspaceLens",
  "TeamMemberRouteTab",
  "TeamRouteState",
  "WorkspaceLens",
] as const;

const TEAM_ROUTE_SOURCE_ALLOWLIST = new Set([
  "./team_route_helpers.ts",
  "./team_route_helpers.test.ts",
  "./team_route_boundary.test.ts",
]);

const sourceModules = import.meta.glob<string>(
  [
    "../**/*.{ts,tsx}",
    "../../components/agent_nodes_workbench.tsx",
    "../../../tests/e2e/**/*.{ts,tsx}",
  ],
  {
    query: "?raw",
    import: "default",
    eager: true,
  }
);

const directNavigationModules = import.meta.glob<string>(
  [
    "../team_page.tsx",
    "../team/**/*.{ts,tsx}",
    "../../routes/use_workspace_route_state.ts",
    "../../components/workbench_header_menu.tsx",
    "../../components/agent_nodes_workbench.tsx",
    "../../../tests/e2e/**/*.{ts,tsx}",
  ],
  {
    query: "?raw",
    import: "default",
    eager: true,
  }
);

const productionNavigationModules = import.meta.glob<string>(
  [
    "../team_page.tsx",
    "../team/**/*.{ts,tsx}",
    "../../routes/use_workspace_route_state.ts",
    "../../components/workbench_header_menu.tsx",
    "../../components/agent_nodes_workbench.tsx",
  ],
  {
    query: "?raw",
    import: "default",
    eager: true,
  }
);

const appModule = import.meta.glob<string>("../../app.tsx", {
  query: "?raw",
  import: "default",
  eager: true,
});

function importedNamesFromAppRouteSelection(source: string): Set<string> {
  const names = new Set<string>();
  const importPattern = /import\s+(?:type\s+)?\{(?<imports>[\s\S]*?)\}\s+from\s+["'][^"']*app_route_selection["']/g;
  for (const match of source.matchAll(importPattern)) {
    const imports = match.groups?.imports ?? "";
    for (const segment of imports.split(",")) {
      const imported = segment.trim().split(/\s+as\s+/)[0]?.trim();
      if (imported) {
        names.add(imported);
      }
    }
  }
  return names;
}

function exportedNamesFromAppRouteSelection(source: string): Set<string> {
  const names = new Set<string>();
  const exportPattern = /export\s+\{(?<exports>[\s\S]*?)\}\s+from\s+["'][^"']*app_route_selection["']/g;
  for (const match of source.matchAll(exportPattern)) {
    const exports = match.groups?.exports ?? "";
    for (const segment of exports.split(",")) {
      const exported = segment.trim().split(/\s+as\s+/)[0]?.trim();
      if (exported) {
        names.add(exported);
      }
    }
  }
  return names;
}

describe("Team route facade boundary", () => {
  it("keeps global Team route symbols behind team_route_helpers", () => {
    const violations = Object.entries(sourceModules).flatMap(([path, source]) => {
      if (TEAM_ROUTE_SOURCE_ALLOWLIST.has(path)) {
        return [];
      }
      const importedNames = importedNamesFromAppRouteSelection(source);
      const routeImports = TEAM_ROUTE_SYMBOLS.filter((symbol) => importedNames.has(symbol));
      return routeImports.length === 0
        ? []
        : [`${path}: ${routeImports.join(", ")}`];
    });

    expect(violations).toEqual([]);
  });

  it("keeps app-level exports from re-exposing Team route symbols", () => {
    const violations = Object.entries(appModule).flatMap(([path, source]) => {
      const exportedNames = exportedNamesFromAppRouteSelection(source);
      const routeExports = TEAM_ROUTE_SYMBOLS.filter((symbol) => exportedNames.has(symbol));
      return routeExports.length === 0
        ? []
        : [`${path}: ${routeExports.join(", ")}`];
    });

    expect(violations).toEqual([]);
  });

  it("keeps Team navigation entrypoints behind named route helpers", () => {
    const directTeamGotos = Object.entries(directNavigationModules).flatMap(([path, source]) => {
      const matches = source.matchAll(
        /(?:page\.goto|onNavigate|navigateWorkbenchRoute)\(\s*(["'`])\/(?:workspace\/)?teams(?:\/|\?|#|\1)/g
      );
      return Array.from(matches, (match) => `${path}: ${match[0]}`);
    });

    expect(directTeamGotos).toEqual([]);
  });

  it("keeps production Team links behind named route helpers", () => {
    const directTeamLinks = Object.entries(productionNavigationModules).flatMap(([path, source]) => {
      if (TEAM_ROUTE_SOURCE_ALLOWLIST.has(path)) {
        return [];
      }
      const matches = source.matchAll(
        /(?:href|to)=\{?\s*(["'`])\/(?:workspace\/)?teams(?:\/|\?|#|\1)/g
      );
      return Array.from(matches, (match) => `${path}: ${match[0]}`);
    });

    expect(directTeamLinks).toEqual([]);
  });

  it("keeps positional canonical Team subpath building inside the facade", () => {
    const violations = Object.entries(productionNavigationModules).flatMap(([path, source]) => {
      if (TEAM_ROUTE_SOURCE_ALLOWLIST.has(path)) {
        return [];
      }
      const imports = source.matchAll(
        /import\s+(?:type\s+)?\{(?<imports>[\s\S]*?)\}\s+from\s+["'][^"']*team_route_helpers["']/g
      );
      const imported = Array.from(imports).some((match) =>
        (match.groups?.imports ?? "")
          .split(",")
          .some((segment) => segment.trim().split(/\s+as\s+/)[0] === "buildCanonicalTeamSubpath")
      );
      return imported ? [`${path}: buildCanonicalTeamSubpath`] : [];
    });

    expect(violations).toEqual([]);
  });
});
