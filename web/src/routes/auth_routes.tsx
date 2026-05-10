import React, { useEffect } from "react";
import { clearAuthAndRedirect } from "../auth_redirect";
import { AUTH_CARD_BASE_CLASS, AUTH_PAGE_CLASS } from "../ui/tailwind_classes";

export const LazyJoinPage = React.lazy(async () => {
  const module = (await import("../pages/join_page")) as typeof import("../pages/join_page");
  return { default: module.JoinPage };
});

export function AuthRedirect(): null {
  useEffect(() => {
    clearAuthAndRedirect(`${location.pathname}${location.search}${location.hash}`);
  }, []);
  return null;
}

export function PostLoginRedirect({ target }: { target: string }): null {
  useEffect(() => {
    location.replace(target);
  }, [target]);
  return null;
}

export function AuthGateCard({ title, message }: { title: string; message: string }) {
  return (
    <div className={AUTH_PAGE_CLASS}>
      <section className={AUTH_CARD_BASE_CLASS}>
        <h2 className="text-xl font-semibold tracking-tight text-notion-text">{title}</h2>
        <p className="mt-2 text-sm text-notion-text-muted">{message}</p>
      </section>
    </div>
  );
}

export function AuthRequiredGate() {
  return <AuthGateCard title="Login Required" message="Please login to continue." />;
}

export function ForbiddenRoute() {
  return (
    <AuthGateCard
      title="Forbidden"
      message="You do not have access to this page."
    />
  );
}
