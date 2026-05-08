// @vitest-environment jsdom
import { act, createRef } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthState } from "../types";
import { TeamRouteContainer } from "./team_route_container";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const teamPageProps: Array<Record<string, unknown>> = [];

vi.mock("../pages/team_page", () => ({
  TeamPage: (props: Record<string, unknown>) => {
    teamPageProps.push(props);
    return <div data-testid="team-page">team:{String(props.routeTeamId)}</div>;
  },
}));

describe("TeamRouteContainer", () => {
  let container: HTMLDivElement;
  let root: Root;

  const auth: AuthState = {
    token: "token-1",
    userId: "user-1",
    username: "hawkingrei",
    role: "admin",
  };

  beforeEach(() => {
    teamPageProps.length = 0;
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it("keeps Team route parsing and page props out of the app shell", async () => {
    const appRootRef = createRef<HTMLDivElement>();
    const onLogout = vi.fn();

    await act(async () => {
      root.render(
        <TeamRouteContainer
          appRootRef={appRootRef}
          auth={auth}
          developerMode={true}
          defaultWorktreeRoot="/tmp/work"
          routePathname="/workspace/teams/team-1"
          routeSearch="?lens=channels&channel=ops"
          onLogout={onLogout}
        />
      );
      await vi.dynamicImportSettled();
    });

    expect(container.textContent).toContain("team:team-1");
    expect(appRootRef.current).toBe(container.firstElementChild);
    expect(teamPageProps[teamPageProps.length - 1]).toMatchObject({
      auth,
      token: "token-1",
      developerMode: true,
      routeTeamId: "team-1",
      routeSearch: "?lens=channels&channel=ops",
      defaultWorktreeRoot: "/tmp/work",
      onLogout,
    });
  });
});
