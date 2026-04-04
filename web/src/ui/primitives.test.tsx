import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  ActionButton,
  IconButton,
  PanelHeader,
  StatusPill,
  SurfaceCard,
} from "./primitives";

describe("ui primitives", () => {
  it("renders shared surface card shell", () => {
    const html = renderToStaticMarkup(
      <SurfaceCard className="p-4">body</SurfaceCard>
    );
    expect(html).toContain("rounded-xl border border-notion-border bg-white");
    expect(html).toContain("p-4");
  });

  it("renders panel header with shared title and actions layout", () => {
    const html = renderToStaticMarkup(
      <PanelHeader
        title="Title"
        subtitle="Subtitle"
        actions={<ActionButton tone="secondary">Refresh</ActionButton>}
      />
    );
    expect(html).toContain("border-b border-notion-border/60 pb-4");
    expect(html).toContain("text-lg font-bold tracking-tight text-notion-text");
    expect(html).toContain("Refresh");
  });

  it("renders action button tone variants", () => {
    const primaryHtml = renderToStaticMarkup(
      <ActionButton tone="primary">Create</ActionButton>
    );
    const dangerHtml = renderToStaticMarkup(
      <ActionButton tone="danger" size="sm">
        Delete
      </ActionButton>
    );
    expect(primaryHtml).toContain("bg-notion-accent text-white");
    expect(dangerHtml).toContain("border border-red-200 bg-red-50 text-red-600");
    expect(dangerHtml).toContain("h-8 px-3 text-[12px]");
  });

  it("renders icon button and status pill primitives", () => {
    const iconHtml = renderToStaticMarkup(
      <IconButton tone="active" size="sm" aria-label="Open">
        <i className="bi bi-plus" aria-hidden="true" />
      </IconButton>
    );
    const pillHtml = renderToStaticMarkup(<StatusPill>Leader</StatusPill>);
    expect(iconHtml).toContain("bg-notion-accent-bg text-notion-accent");
    expect(iconHtml).toContain("h-7 w-7");
    expect(pillHtml).toContain("rounded-full border border-notion-border");
    expect(pillHtml).toContain("Leader");
  });
});
