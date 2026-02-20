import React from "react";

const AUTH_PAGE_CLASS =
  "app min-h-[var(--agenthub-vh,100vh)] px-4 py-8 md:px-6 md:py-10";
const AUTH_CARD_CLASS =
  "auth mx-auto w-full max-w-md rounded-2xl border border-slate-200/80 bg-white/90 p-6 shadow-sm backdrop-blur";

export function AuthRequired() {
  return (
    <div className={AUTH_PAGE_CLASS}>
      <section className={AUTH_CARD_CLASS}>
        <h2 className="text-xl font-semibold tracking-tight text-slate-900">Login Required</h2>
        <p className="mt-2 text-sm text-slate-600">Please login to continue.</p>
      </section>
    </div>
  );
}

export function ForbiddenPage() {
  return (
    <div className={AUTH_PAGE_CLASS}>
      <section className={AUTH_CARD_CLASS}>
        <h2 className="text-xl font-semibold tracking-tight text-slate-900">Forbidden</h2>
        <p className="mt-2 text-sm text-slate-600">You do not have access to this page.</p>
      </section>
    </div>
  );
}
