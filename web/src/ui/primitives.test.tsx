import { MantineProvider } from "@mantine/core";
import type React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  ActionButton,
  Badge,
  CompactButton,
  CompactIconButton,
  ConversationBubble,
  EmptyState,
  IconButton,
  InsetSurface,
  KeyValueItem,
  KeyValueList,
  MenuOptionButton,
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
    expect(primaryHtml).not.toContain("mantine-UnstyledButton-root");
    expect(dangerHtml).toContain("border border-state-error-border bg-state-error-bg text-state-error-text");
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
    expect(activeHtml).toContain(
      "focus-visible:ring-2 focus-visible:ring-notion-accent focus-visible:ring-offset-2"
    );
    expect(activeHtml).not.toContain("mantine-UnstyledButton-root");
    expect(idleHtml).not.toContain("ring-1 ring-notion-accent/30");
    expect(activeHtml).toContain('type="button"');
  });

  it("renders empty states and key/value metadata primitives", () => {
    const emptyHtml = renderHtml(
      <EmptyState title="No messages yet" body="Start the conversation from the channel composer." />
    );
    const metadataHtml = renderHtml(
      <KeyValueList>
        <KeyValueItem label="source" value="group_chat" data-testid="meta-source" />
        <KeyValueItem label="from" value="leader-agent" valueClassName="mono" />
      </KeyValueList>
    );

    expect(emptyHtml).toContain("border-dashed border-notion-border/80");
    expect(emptyHtml).toContain("No messages yet");
    expect(emptyHtml).toContain("Start the conversation from the channel composer.");
    expect(metadataHtml).toContain("grid-cols-[auto,minmax(0,1fr)]");
    expect(metadataHtml).toContain('data-testid="meta-source"');
    expect(metadataHtml).toContain("leader-agent");
    expect(metadataHtml).toContain("mono");
  });

  it("renders conversation bubbles and compact menu option buttons", () => {
    const bubbleHtml = renderHtml(
      <ConversationBubble className="border-notion-border-subtle bg-white">Bubble</ConversationBubble>
    );
    const optionHtml = renderHtml(
      <MenuOptionButton active data-testid="mention-worker">
        Worker
      </MenuOptionButton>
    );

    expect(bubbleHtml).toContain("rounded-[18px] border px-3.5 py-2.25 shadow-notion-soft");
    expect(bubbleHtml).toContain("Bubble");
    expect(optionHtml).toContain("bg-brand-primary/10 text-brand-primary");
    expect(optionHtml).toContain('data-testid="mention-worker"');
  });

  it("renders compact action primitives for dense toolbars", () => {
    const compactButtonHtml = renderHtml(
      <CompactButton data-testid="details-toggle">Show details</CompactButton>
    );
    const compactIconHtml = renderHtml(
      <CompactIconButton aria-label="Seen by 1 recipient">
        <span />
      </CompactIconButton>
    );

    expect(compactButtonHtml).toContain("text-[10px] font-bold uppercase tracking-wider");
    expect(compactButtonHtml).toContain('data-testid="details-toggle"');
    expect(compactIconHtml).toContain("h-5 min-w-5 items-center justify-center");
    expect(compactIconHtml).toContain('aria-label="Seen by 1 recipient"');
  });

  it("renders badge variants for compact status and recipient chips", () => {
    const subtleHtml = renderHtml(
      <Badge className="text-[10px] uppercase tracking-wider">Awaiting human review</Badge>
    );
    const outlineHtml = renderHtml(
      <Badge tone="outline" shape="pill">Worker Agent</Badge>
    );
    const dashedHtml = renderHtml(
      <Badge tone="dashed" shape="pill">Leader Agent</Badge>
    );

    expect(subtleHtml).toContain("bg-notion-hover");
    expect(subtleHtml).toContain("Awaiting human review");
    expect(outlineHtml).toContain("border border-notion-border bg-white");
    expect(outlineHtml).toContain("rounded-full px-2 py-0.5");
    expect(dashedHtml).toContain("border border-dashed border-notion-border bg-transparent");
  });
});
