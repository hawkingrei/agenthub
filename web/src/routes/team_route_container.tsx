import React, { Suspense, useMemo } from "react";
import { resolveTeamRoute } from "../app_route_selection";
import type { AuthState } from "../types";
import { APP_ROOT_CLASS } from "../ui/tailwind_classes";
import { RouteFallback } from "./route_fallback";

const LazyTeamPage = React.lazy(async () => {
  const module = (await import("../pages/team_page")) as typeof import("../pages/team_page");
  return { default: module.TeamPage };
});

type TeamRouteContainerProps = {
  appRootRef: React.RefObject<HTMLDivElement | null>;
  auth: AuthState;
  developerMode: boolean;
  defaultWorktreeRoot: string | null;
  routePathname: string;
  routeSearch: string;
  onLogout: () => void;
};

export const TeamRouteContainer = React.memo(function TeamRouteContainer({
  appRootRef,
  auth,
  developerMode,
  defaultWorktreeRoot,
  routePathname,
  routeSearch,
  onLogout,
}: TeamRouteContainerProps) {
  const teamRoute = useMemo(() => resolveTeamRoute(routePathname), [routePathname]);

  return (
    <div className={APP_ROOT_CLASS} ref={appRootRef}>
      <Suspense fallback={<RouteFallback label="Loading teams..." />}>
        <LazyTeamPage
          auth={auth}
          token={auth.token}
          onLogout={onLogout}
          developerMode={developerMode}
          routeTeamId={teamRoute?.teamId ?? null}
          routePathname={routePathname}
          routeSearch={routeSearch}
          defaultWorktreeRoot={defaultWorktreeRoot}
        />
      </Suspense>
    </div>
  );
});
