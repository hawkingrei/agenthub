import React from "react";

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
    <div className="flex h-screen flex-col overflow-hidden bg-white">
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
