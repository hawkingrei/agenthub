import { MantineProvider } from "@mantine/core";
import type React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  ActionButton,
  IconButton,
  InsetSurface,
  PanelHeader,
  SelectableListItem,
  StatusPill,
  SurfaceCard,
  ToolbarRow,
} from "./primitives";

describe("ui primitives", () => {
  const renderHtml = (node: React.ReactElement) =>
    renderToStaticMarkup(<MantineProvider>{node}</MantineProvider>);

  it("renders shared surface card shell", () => {
    const html = renderHtml(
      <SurfaceCard className="p-4">body</SurfaceCard>
    );
    expect(html).toContain("rounded-xl border border-notion-border bg-white");
    expect(html).toContain("p-4");
  });

  it("renders inset surface and toolbar row primitives", () => {
    const insetHtml = renderHtml(
      <InsetSurface className="teams-run-list">body</InsetSurface>
    );
    const toolbarHtml = renderHtml(
      <ToolbarRow className="border-b border-notion-border/50 pb-4">row</ToolbarRow>
    );
    expect(insetHtml).toContain("rounded-xl border border-notion-border bg-notion-sidebar/10");
    expect(insetHtml).toContain("teams-run-list");
    expect(toolbarHtml).toContain("flex flex-wrap items-center justify-between gap-3");
    expect(toolbarHtml).toContain("border-b border-notion-border/50 pb-4");
  });

  it("renders panel header with shared title and actions layout", () => {
    const html = renderHtml(
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
    const primaryHtml = renderHtml(
      <ActionButton tone="primary">Create</ActionButton>
    );
    const dangerHtml = renderHtml(
      <ActionButton tone="danger" size="sm">
        Delete
      </ActionButton>
    );
    expect(primaryHtml).toContain("bg-notion-accent text-white");
    expect(dangerHtml).toContain("border border-red-200 bg-red-50 text-red-600");
    expect(dangerHtml).toContain("h-8 px-3 text-[12px]");
  });

  it("renders icon button and status pill primitives", () => {
    const iconHtml = renderHtml(
      <IconButton tone="active" size="sm" aria-label="Open">
        <i className="bi bi-plus" aria-hidden="true" />
      </IconButton>
    );
    const pillHtml = renderHtml(<StatusPill>Leader</StatusPill>);
    expect(iconHtml).toContain("bg-notion-accent-bg text-notion-accent");
    expect(iconHtml).toContain("h-7 w-7");
    expect(pillHtml).toContain("rounded-full border border-notion-border");
    expect(pillHtml).toContain("Leader");
  });

  it("renders selectable list items with active state", () => {
    const activeHtml = renderHtml(
      <SelectableListItem active>Item</SelectableListItem>
    );
    const idleHtml = renderHtml(
      <SelectableListItem>Item</SelectableListItem>
    );
    expect(activeHtml).toContain("team-item");
    expect(activeHtml).toContain("ring-1 ring-notion-accent/30");
    expect(idleHtml).not.toContain("ring-1 ring-notion-accent/30");
    expect(activeHtml).toContain('type="button"');
  });
});
