import React from "react";

export function AuthRequired() {
  return (
    <div className="app">
      <section className="auth">
        <h2>Login Required</h2>
        <p>Please login to continue.</p>
      </section>
    </div>
  );
}

export function ForbiddenPage() {
  return (
    <div className="app">
      <section className="auth">
        <h2>Forbidden</h2>
        <p>You do not have access to this page.</p>
      </section>
    </div>
  );
}
