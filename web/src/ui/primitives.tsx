import { ActionIcon, Box } from "@mantine/core";
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
  "team-item group relative flex w-full flex-col gap-1.5 rounded-xl border border-notion-border bg-white p-4 text-left shadow-sm transition-all hover:border-notion-accent/20 hover:bg-notion-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-notion-accent focus-visible:ring-offset-2 focus-visible:ring-offset-white active:translate-y-px disabled:cursor-not-allowed disabled:opacity-60";
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
    "border border-state-error-border bg-state-error-bg text-state-error-text shadow-sm hover:bg-state-error-bg/80",
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
  danger: "text-notion-text-muted hover:bg-state-error-bg hover:text-state-error-text",
} as const;

const STATUS_PILL_BASE_CLASS =
  "inline-flex shrink-0 items-center rounded-full border border-notion-border bg-white px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted";
const EMPTY_STATE_BASE_CLASS =
  "rounded-xl border border-dashed border-notion-border bg-notion-sidebar/20 px-4 py-5 text-sm text-notion-text-muted";
const EMPTY_STATE_TITLE_CLASS = "text-sm font-semibold text-notion-text";
const EMPTY_STATE_BODY_CLASS = "mt-1 leading-relaxed text-notion-text-muted";
const INLINE_NOTICE_BASE_CLASS =
  "rounded-lg border px-3 py-2 text-sm leading-relaxed";
const INLINE_NOTICE_TONE_CLASS = {
  info: "border-notion-border bg-notion-sidebar/20 text-notion-text-muted",
  warning:
    "border-state-warning-border bg-state-warning-bg/60 text-state-warning-text",
  danger: "border-state-error-border bg-state-error-bg/60 text-state-error-text",
} as const;
const KEY_VALUE_LIST_BASE_CLASS =
  "grid min-w-0 gap-x-3 gap-y-1 text-[12px] leading-relaxed text-notion-text-muted sm:grid-cols-[auto_minmax(0,1fr)]";
const KEY_VALUE_ITEM_BASE_CLASS = "contents";
const KEY_VALUE_LABEL_CLASS =
  "font-bold uppercase tracking-wider text-[10px] text-notion-text-muted/80";
const KEY_VALUE_VALUE_CLASS = "min-w-0 break-words text-notion-text";

type SurfaceCardProps = React.ComponentPropsWithoutRef<typeof Box>;

export function SurfaceCard({ className, ...props }: SurfaceCardProps) {
  return <Box className={cx(SURFACE_CARD_BASE_CLASS, className)} {...props} />;
}

type InsetSurfaceProps = React.ComponentPropsWithoutRef<typeof Box>;

export function InsetSurface({ className, ...props }: InsetSurfaceProps) {
  return <Box className={cx(INSET_SURFACE_BASE_CLASS, className)} {...props} />;
}

type ToolbarRowProps = React.ComponentPropsWithoutRef<typeof Box>;

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
    <Box className={cx(PANEL_HEADER_ROOT_CLASS, className)}>
      <Box className={cx(PANEL_HEADER_CONTENT_CLASS, contentClassName)}>
        <Box className={cx(PANEL_HEADER_TITLE_CLASS, titleClassName)}>{title}</Box>
        {subtitle ? (
          <Box className={cx(PANEL_HEADER_SUBTITLE_CLASS, subtitleClassName)}>
            {subtitle}
          </Box>
        ) : null}
      </Box>
      {actions ? (
        <Box className={cx(PANEL_HEADER_ACTIONS_CLASS, actionsClassName)}>{actions}</Box>
      ) : null}
    </Box>
  );
}

type SelectableListItemProps = React.ComponentPropsWithoutRef<"button"> & {
  active?: boolean;
  layout?: "column" | "row";
};

export const SelectableListItem = React.forwardRef<HTMLButtonElement, SelectableListItemProps>(
  function SelectableListItem(
    { active = false, className, layout = "column", type = "button", ...props },
    ref
  ) {
    return (
      <Box
        ref={ref}
        component="button"
        type={type}
        className={cx(
          SELECTABLE_LIST_ITEM_BASE_CLASS,
          layout === "row" ? "flex-row items-start" : "flex-col",
          active && SELECTABLE_LIST_ITEM_ACTIVE_CLASS,
          className
        )}
        {...props}
      />
    );
  }
);
SelectableListItem.displayName = "SelectableListItem";

type ActionButtonProps = React.ComponentPropsWithoutRef<"button"> & {
  tone?: keyof typeof ACTION_BUTTON_TONE_CLASS;
  size?: keyof typeof ACTION_BUTTON_SIZE_CLASS;
};

export const ActionButton = React.forwardRef<HTMLButtonElement, ActionButtonProps>(
  function ActionButton(
    { tone = "secondary", size = "md", className, type = "button", ...props },
    ref
  ) {
    return (
      <Box
        ref={ref}
        component="button"
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
);
ActionButton.displayName = "ActionButton";

type IconButtonProps = React.ComponentPropsWithoutRef<typeof ActionIcon> & {
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
    <ActionIcon
      unstyled
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

type EmptyStateProps = React.ComponentPropsWithoutRef<typeof Box> & {
  title?: React.ReactNode;
  body?: React.ReactNode;
};

export function EmptyState({ title, body, className, children, ...props }: EmptyStateProps) {
  return (
    <Box className={cx(EMPTY_STATE_BASE_CLASS, className)} {...props}>
      {title ? <Box className={EMPTY_STATE_TITLE_CLASS}>{title}</Box> : null}
      {body ? <Box className={EMPTY_STATE_BODY_CLASS}>{body}</Box> : null}
      {children}
    </Box>
  );
}

type InlineNoticeProps = React.ComponentPropsWithoutRef<typeof Box> & {
  tone?: keyof typeof INLINE_NOTICE_TONE_CLASS;
};

export function InlineNotice({
  tone = "info",
  className,
  ...props
}: InlineNoticeProps) {
  return (
    <Box
      className={cx(INLINE_NOTICE_BASE_CLASS, INLINE_NOTICE_TONE_CLASS[tone], className)}
      {...props}
    />
  );
}

type KeyValueListProps = React.ComponentPropsWithoutRef<typeof Box>;

export function KeyValueList({ className, ...props }: KeyValueListProps) {
  return <Box className={cx(KEY_VALUE_LIST_BASE_CLASS, className)} {...props} />;
}

type KeyValueItemProps = {
  label: React.ReactNode;
  value: React.ReactNode;
  className?: string;
  labelClassName?: string;
  valueClassName?: string;
} & Omit<React.ComponentPropsWithoutRef<typeof Box>, "children">;

export function KeyValueItem({
  label,
  value,
  className,
  labelClassName,
  valueClassName,
  ...props
}: KeyValueItemProps) {
  return (
    <Box className={cx(KEY_VALUE_ITEM_BASE_CLASS, className)} {...props}>
      <Box component="span" className={cx(KEY_VALUE_LABEL_CLASS, labelClassName)}>
        {label}
      </Box>
      <Box component="span" className={cx(KEY_VALUE_VALUE_CLASS, valueClassName)}>
        {value}
      </Box>
    </Box>
  );
}
