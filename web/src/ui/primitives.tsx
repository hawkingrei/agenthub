import { Box, UnstyledButton } from "@mantine/core";
import React from "react";

export function cx(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ");
}

const SURFACE_CARD_BASE_CLASS =
  "min-h-0 rounded-xl border border-notion-border bg-white shadow-sm transition-all";
const INSET_SURFACE_BASE_CLASS =
  "min-h-0 rounded-xl border border-notion-border bg-notion-sidebar/10 p-4 sm:p-6";
const TOOLBAR_ROW_BASE_CLASS =
  "flex flex-wrap items-center justify-between gap-3";
const SELECTABLE_LIST_ITEM_BASE_CLASS =
  "team-item group relative flex w-full flex-col gap-1.5 rounded-xl border border-notion-border bg-white p-4 text-left shadow-sm transition-all hover:border-notion-accent/20 hover:bg-notion-hover active:translate-y-px disabled:cursor-not-allowed disabled:opacity-60";
const SELECTABLE_LIST_ITEM_ACTIVE_CLASS =
  "border-notion-accent/30 bg-notion-hover shadow-md ring-1 ring-notion-accent/30";

const PANEL_HEADER_ROOT_CLASS =
  "flex flex-wrap items-start justify-between gap-3 border-b border-notion-border/60 pb-4";
const PANEL_HEADER_CONTENT_CLASS = "min-w-0 flex flex-1 flex-col gap-1";
const PANEL_HEADER_TITLE_CLASS = "text-lg font-bold tracking-tight text-notion-text";
const PANEL_HEADER_SUBTITLE_CLASS =
  "text-[13px] leading-relaxed text-notion-text-muted";
const PANEL_HEADER_ACTIONS_CLASS =
  "flex shrink-0 flex-wrap items-center gap-2 justify-end";

const ACTION_BUTTON_BASE_CLASS =
  "inline-flex items-center justify-center gap-1.5 rounded-lg font-semibold transition active:translate-y-px disabled:cursor-not-allowed disabled:opacity-50";

const ACTION_BUTTON_SIZE_CLASS = {
  sm: "h-8 px-3 text-[12px]",
  md: "h-9 px-4 text-[13px]",
} as const;

const ACTION_BUTTON_TONE_CLASS = {
  primary: "bg-notion-accent text-white shadow-sm hover:bg-notion-accent/90",
  secondary:
    "border border-notion-border bg-white text-notion-text shadow-sm hover:bg-notion-hover",
  ghost: "bg-transparent text-notion-text-muted hover:bg-notion-hover hover:text-notion-text",
  danger:
    "border border-red-200 bg-red-50 text-red-600 shadow-sm hover:bg-red-100",
} as const;

const ICON_BUTTON_BASE_CLASS =
  "inline-flex items-center justify-center rounded-md transition active:translate-y-px disabled:cursor-not-allowed disabled:opacity-50";

const ICON_BUTTON_SIZE_CLASS = {
  sm: "h-7 w-7 text-[13px] sm:h-8 sm:w-8 sm:text-[14px]",
  md: "h-8 w-8 sm:h-9 sm:w-9",
} as const;

const ICON_BUTTON_TONE_CLASS = {
  default:
    "border border-notion-border bg-white text-notion-text-muted shadow-sm hover:bg-notion-hover hover:text-notion-text",
  active:
    "border border-notion-accent/30 bg-notion-accent-bg text-notion-accent shadow-sm hover:bg-notion-accent/10",
  subtle:
    "text-notion-text-muted hover:bg-notion-hover hover:text-notion-text",
  danger: "text-notion-text-muted hover:bg-red-50 hover:text-red-600",
} as const;

const STATUS_PILL_BASE_CLASS =
  "inline-flex shrink-0 items-center rounded-full border border-notion-border bg-white px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted";

type SurfaceCardProps = React.HTMLAttributes<HTMLDivElement>;

export function SurfaceCard({ className, ...props }: SurfaceCardProps) {
  return <div className={cx(SURFACE_CARD_BASE_CLASS, className)} {...props} />;
}

type InsetSurfaceProps = React.HTMLAttributes<HTMLDivElement>;

export function InsetSurface({ className, ...props }: InsetSurfaceProps) {
  return <Box className={cx(INSET_SURFACE_BASE_CLASS, className)} {...props} />;
}

type ToolbarRowProps = React.HTMLAttributes<HTMLDivElement>;

export function ToolbarRow({ className, ...props }: ToolbarRowProps) {
  return <Box className={cx(TOOLBAR_ROW_BASE_CLASS, className)} {...props} />;
}

type PanelHeaderProps = {
  title: React.ReactNode;
  subtitle?: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
  contentClassName?: string;
  titleClassName?: string;
  subtitleClassName?: string;
  actionsClassName?: string;
};

export function PanelHeader({
  title,
  subtitle,
  actions,
  className,
  contentClassName,
  titleClassName,
  subtitleClassName,
  actionsClassName,
}: PanelHeaderProps) {
  return (
    <div className={cx(PANEL_HEADER_ROOT_CLASS, className)}>
      <div className={cx(PANEL_HEADER_CONTENT_CLASS, contentClassName)}>
        <div className={cx(PANEL_HEADER_TITLE_CLASS, titleClassName)}>{title}</div>
        {subtitle ? (
          <div className={cx(PANEL_HEADER_SUBTITLE_CLASS, subtitleClassName)}>
            {subtitle}
          </div>
        ) : null}
      </div>
      {actions ? (
        <div className={cx(PANEL_HEADER_ACTIONS_CLASS, actionsClassName)}>{actions}</div>
      ) : null}
    </div>
  );
}

type SelectableListItemProps = React.ComponentPropsWithoutRef<typeof UnstyledButton> & {
  active?: boolean;
};

export function SelectableListItem({
  active = false,
  className,
  ...props
}: SelectableListItemProps) {
  return (
    <UnstyledButton
      className={cx(
        SELECTABLE_LIST_ITEM_BASE_CLASS,
        active && SELECTABLE_LIST_ITEM_ACTIVE_CLASS,
        className
      )}
      {...props}
    />
  );
}

type ActionButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  tone?: keyof typeof ACTION_BUTTON_TONE_CLASS;
  size?: keyof typeof ACTION_BUTTON_SIZE_CLASS;
};

export function ActionButton({
  tone = "secondary",
  size = "md",
  className,
  type = "button",
  ...props
}: ActionButtonProps) {
  return (
    <button
      type={type}
      className={cx(
        ACTION_BUTTON_BASE_CLASS,
        ACTION_BUTTON_SIZE_CLASS[size],
        ACTION_BUTTON_TONE_CLASS[tone],
        className
      )}
      {...props}
    />
  );
}

type IconButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  tone?: keyof typeof ICON_BUTTON_TONE_CLASS;
  size?: keyof typeof ICON_BUTTON_SIZE_CLASS;
};

export function IconButton({
  tone = "default",
  size = "md",
  className,
  type = "button",
  ...props
}: IconButtonProps) {
  return (
    <button
      type={type}
      className={cx(
        ICON_BUTTON_BASE_CLASS,
        ICON_BUTTON_SIZE_CLASS[size],
        ICON_BUTTON_TONE_CLASS[tone],
        className
      )}
      {...props}
    />
  );
}

type StatusPillProps = React.HTMLAttributes<HTMLSpanElement>;

export function StatusPill({ className, ...props }: StatusPillProps) {
  return <span className={cx(STATUS_PILL_BASE_CLASS, className)} {...props} />;
}
