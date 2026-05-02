import { AUTH_CARD_BASE_CLASS, AUTH_PAGE_CLASS } from "../ui/tailwind_classes";

export function AuthRequired() {
  return (
    <div className={AUTH_PAGE_CLASS}>
      <section className={AUTH_CARD_BASE_CLASS}>
        <h2 className="text-xl font-semibold tracking-tight text-notion-text">Login Required</h2>
        <p className="mt-2 text-sm text-notion-text-muted">Please login to continue.</p>
      </section>
    </div>
  );
}

export function ForbiddenPage() {
  return (
    <div className={AUTH_PAGE_CLASS}>
      <section className={AUTH_CARD_BASE_CLASS}>
        <h2 className="text-xl font-semibold tracking-tight text-notion-text">Forbidden</h2>
        <p className="mt-2 text-sm text-notion-text-muted">You do not have access to this page.</p>
      </section>
    </div>
  );
}
