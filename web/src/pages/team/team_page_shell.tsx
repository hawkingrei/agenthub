import React from "react";
import { TEAM_PAGE_SHELL_ROOT_CLASS } from "../../ui/tailwind_classes";

type TeamPageShellProps = {
  header: React.ReactNode;
  errorBanner?: React.ReactNode;
  warningNotice?: React.ReactNode;
  isSelectorRoute: boolean;
  selectorContent: React.ReactNode;
  detailLayoutClassName: string;
  showSidebarPane: boolean;
  sidebarPane: React.ReactNode;
  showWorkbenchPane: boolean;
  workbenchPane: React.ReactNode;
};

export const TeamPageShell = React.memo(function TeamPageShell({
  header,
  errorBanner = null,
  warningNotice = null,
  isSelectorRoute,
  selectorContent,
  detailLayoutClassName,
  showSidebarPane,
  sidebarPane,
  showWorkbenchPane,
  workbenchPane,
}: TeamPageShellProps) {
  return (
    <div className={TEAM_PAGE_SHELL_ROOT_CLASS}>
      {header}
      {errorBanner}
      {warningNotice}
      {isSelectorRoute ? (
        selectorContent
      ) : (
        <div className={detailLayoutClassName}>
          {showSidebarPane ? sidebarPane : null}
          {showWorkbenchPane ? workbenchPane : null}
        </div>
      )}
    </div>
  );
});
