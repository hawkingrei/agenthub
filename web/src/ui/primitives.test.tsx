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
  InlineNotice,
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
    const html = renderHtml(<SurfaceCard className="p-4">body</SurfaceCard>);
    expect(html).toContain("rounded-xl border border-notion-border bg-white");
    expect(html).toContain("p-4");
  });

  it("renders inset surface and toolbar row primitives", () => {
    const insetHtml = renderHtml(<InsetSurface className="teams-run-list">body</InsetSurface>);
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
    const primaryHtml = renderHtml(<ActionButton tone="primary">Create</ActionButton>);
    const dangerHtml = renderHtml(
      <ActionButton tone="danger" size="sm">
        Delete
      </ActionButton>
    );
    expect(primaryHtml).toContain("bg-notion-accent text-white");
    expect(primaryHtml).not.toContain("mantine-UnstyledButton-root");
    expect(dangerHtml).toContain(
      "border border-state-error-border bg-state-error-bg text-state-error-text"
    );
    expect(dangerHtml).toContain("h-8 px-3 text-[12px]");
  });

  it("renders icon button and status pill primitives", () => {
    const iconHtml = renderHtml(
      <IconButton tone="active" size="sm" aria-label="Open">
        <i className="bi bi-plus" aria-hidden="true" />
      </IconButton>
    );
    const pillHtml = renderHtml(
      <StatusPill className="border-notion-border bg-white text-notion-text-muted">
        Leader
      </StatusPill>
    );
    expect(iconHtml).toContain("bg-notion-accent-bg text-notion-accent");
    expect(iconHtml).toContain("h-7 w-7");
    expect(pillHtml).toContain("rounded-full border px-2 py-0.5");
    expect(pillHtml).toContain("border-notion-border bg-white text-notion-text-muted");
    expect(pillHtml).toContain("Leader");
  });

  it("renders selectable list items with active state", () => {
    const activeHtml = renderHtml(<SelectableListItem active>Item</SelectableListItem>);
    const idleHtml = renderHtml(<SelectableListItem>Item</SelectableListItem>);
    expect(activeHtml).toContain("team-item");
    expect(activeHtml).toContain("ring-1 ring-notion-accent/30");
    expect(activeHtml).toContain(
      "focus-visible:ring-2 focus-visible:ring-notion-accent focus-visible:ring-offset-2"
    );
    expect(activeHtml).not.toContain("mantine-UnstyledButton-root");
    expect(idleHtml).not.toContain("ring-1 ring-notion-accent/30");
    expect(activeHtml).toContain('type="button"');
  });

  it("renders empty state, inline notice, and key/value metadata primitives", () => {
    const emptyHtml = renderHtml(<EmptyState title="No steps" body="Nothing has executed yet." />);
    const noticeHtml = renderHtml(
      <InlineNotice tone="warning">Developer Mode only.</InlineNotice>
    );
    const keyValueHtml = renderHtml(
      <KeyValueList>
        <KeyValueItem label="role" value="leader" data-testid="role-row" />
      </KeyValueList>
    );
    expect(emptyHtml).toContain("border-dashed border-notion-border");
    expect(emptyHtml).toContain("No steps");
    expect(noticeHtml).toContain("border-state-warning-border bg-state-warning-bg/60");
    expect(noticeHtml).toContain("Developer Mode only.");
    expect(keyValueHtml).toContain("grid min-w-0 gap-x-3 gap-y-1");
    expect(keyValueHtml).toContain("<dl");
    expect(keyValueHtml).toContain("<dt");
    expect(keyValueHtml).toContain("<dd");
    expect(keyValueHtml).toContain(
      "text-[10px] font-bold uppercase tracking-wider text-notion-text-muted/80"
    );
    expect(keyValueHtml).toContain(
      "min-w-0 break-words text-[12px] leading-relaxed text-notion-text"
    );
    expect(keyValueHtml).toContain("role");
    expect(keyValueHtml).toContain("leader");
    expect(keyValueHtml).toContain('data-testid="role-row"');
  });

  it("forwards box props through KeyValueItem without exposing children in the API", () => {
    const html = renderHtml(
      <KeyValueList>
        <KeyValueItem
          label="current"
          value="collecting evidence"
          title="collecting evidence"
          id="current-row"
        />
      </KeyValueList>
    );
    expect(html).toContain('id="current-row"');
    expect(html).toContain('title="collecting evidence"');
    expect(html).toContain("collecting evidence");
  });

  it("keeps key-value and status pill primitives structure-first so callers can own tone classes", () => {
    const keyValueHtml = renderHtml(
      <KeyValueList className="text-[11px] text-brand-primary">
        <KeyValueItem
          label="route"
          value="to_leader"
          labelClassName="text-brand-primary/70"
          valueClassName="mono text-brand-primary"
        />
      </KeyValueList>
    );
    const pillHtml = renderHtml(
      <StatusPill className="border-state-warning-border bg-state-warning-bg text-state-warning-text">
        idle=1
      </StatusPill>
    );
    expect(keyValueHtml).toContain("text-[11px] text-brand-primary");
    expect(keyValueHtml).toContain("text-brand-primary/70");
    expect(keyValueHtml).toContain("mono text-brand-primary");
    expect(pillHtml).toContain(
      "border-state-warning-border bg-state-warning-bg text-state-warning-text"
    );
    expect(pillHtml).not.toContain("border-notion-border bg-white text-notion-text-muted");
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

    expect(bubbleHtml).toContain(
      "rounded-[12px] border border-transparent px-2 py-1.25 shadow-none"
    );
    expect(bubbleHtml).toContain("Bubble");
    expect(optionHtml).toContain("bg-brand-primary/10 text-brand-primary");
    expect(optionHtml).toContain('data-testid="mention-worker"');
  });

  it("renders compact action primitives for dense toolbars", () => {
    const compactButtonHtml = renderHtml(
      <CompactButton data-testid="details-toggle">Details</CompactButton>
    );
    const compactIconHtml = renderHtml(
      <CompactIconButton aria-label="Seen by 1 recipient">
        <span />
      </CompactIconButton>
    );

    expect(compactButtonHtml).toContain("text-[10px] font-medium tracking-[0.01em]");
    expect(compactButtonHtml).toContain('data-testid="details-toggle"');
    expect(compactIconHtml).toContain("h-5 min-w-5 items-center justify-center");
    expect(compactIconHtml).toContain('aria-label="Seen by 1 recipient"');
  });

  it("renders badge variants for compact status and recipient chips", () => {
    const subtleHtml = renderHtml(
      <Badge className="text-[10px] uppercase tracking-wider">Awaiting human review</Badge>
    );
    const outlineHtml = renderHtml(
      <Badge tone="outline" shape="pill">
        Worker Agent
      </Badge>
    );
    const dashedHtml = renderHtml(
      <Badge tone="dashed" shape="pill">
        Leader Agent
      </Badge>
    );

    expect(subtleHtml).toContain("bg-notion-hover");
    expect(subtleHtml).toContain("<span");
    expect(subtleHtml).toContain("Awaiting human review");
    expect(outlineHtml).toContain("border border-notion-border bg-white");
    expect(outlineHtml).toContain("rounded-full px-2 py-0.5");
    expect(dashedHtml).toContain("border border-dashed border-notion-border bg-transparent");
  });

  it("renders notice, menu, and empty-state fallback branches", () => {
    const idleOptionHtml = renderHtml(<MenuOptionButton>Idle option</MenuOptionButton>);
    const dangerNoticeHtml = renderHtml(
      <InlineNotice tone="danger">Permission request failed.</InlineNotice>
    );
    const childEmptyStateHtml = renderHtml(
      <EmptyState>
        <span data-testid="empty-child">child</span>
      </EmptyState>
    );

    expect(idleOptionHtml).toContain("text-ui-text-primary hover:bg-ui-surface-soft");
    expect(dangerNoticeHtml).toContain("border-state-error-border bg-state-error-bg/60");
    expect(childEmptyStateHtml).toContain('data-testid="empty-child"');
  });
});
