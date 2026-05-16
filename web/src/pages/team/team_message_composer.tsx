import React from "react";
import { ActionButton, MenuOptionButton, ToolbarRow, cx } from "../../ui/primitives";
import type { MentionCandidate } from "./mailbox_helpers";
import {
  TEAM_MESSAGE_COMPOSER_ACTIONS_ROW_CLASS,
  TEAM_MESSAGE_COMPOSER_CONTEXT_CLASS,
  TEAM_MESSAGE_COMPOSER_EDITOR_ROW_CLASS,
  TEAM_MESSAGE_COMPOSER_HELPER_TEXT_CLASS,
  TEAM_MESSAGE_COMPOSER_MENTION_ALIAS_CLASS,
  TEAM_MESSAGE_COMPOSER_MENTION_HINT_CLASS,
  TEAM_MESSAGE_COMPOSER_MENTION_LIST_CLASS,
  TEAM_MESSAGE_COMPOSER_MENTION_MENU_CLASS,
  TEAM_MESSAGE_COMPOSER_SEND_BUTTON_CLASS,
  TEAM_MESSAGE_COMPOSER_SHELL_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
} from "../../ui/tailwind_classes";

export type TeamMessageComposerMentionOption = MentionCandidate;

type TeamMessageComposerProps = {
  textareaRef?: React.Ref<HTMLTextAreaElement>;
  id?: string;
  name?: string;
  draft: string;
  rows?: number;
  placeholder: string;
  contextText?: string;
  helperText: string;
  sendLabel: string;
  busyLabel?: string;
  busy?: boolean;
  disabled?: boolean;
  className?: string;
  textareaClassName?: string;
  sendButtonClassName?: string;
  mentionOptions?: TeamMessageComposerMentionOption[];
  activeMentionIndex?: number;
  onSelectMention?: (option: TeamMessageComposerMentionOption) => void;
  onDraftChange: React.ChangeEventHandler<HTMLTextAreaElement>;
  onSend: () => void;
  onTextareaClick?: React.MouseEventHandler<HTMLTextAreaElement>;
  onTextareaKeyUp?: React.KeyboardEventHandler<HTMLTextAreaElement>;
  onTextareaKeyDown?: React.KeyboardEventHandler<HTMLTextAreaElement>;
  onTextareaBlur?: React.FocusEventHandler<HTMLTextAreaElement>;
  onCompositionStart?: React.CompositionEventHandler<HTMLTextAreaElement>;
  onCompositionEnd?: React.CompositionEventHandler<HTMLTextAreaElement>;
};

export function TeamMessageComposer({
  textareaRef,
  id,
  name,
  draft,
  rows = 1,
  placeholder,
  contextText,
  helperText,
  sendLabel,
  busyLabel,
  busy = false,
  disabled = false,
  className,
  textareaClassName,
  sendButtonClassName,
  mentionOptions = [],
  activeMentionIndex = 0,
  onSelectMention,
  onDraftChange,
  onSend,
  onTextareaClick,
  onTextareaKeyUp,
  onTextareaKeyDown,
  onTextareaBlur,
  onCompositionStart,
  onCompositionEnd,
}: TeamMessageComposerProps) {
  const reactId = React.useId();
  const hasMentionOptions = mentionOptions.length > 0 && onSelectMention != null;
  const listboxId = `${id ?? reactId}-mention-listbox`;
  const activeMentionOption = mentionOptions[activeMentionIndex] ?? mentionOptions[0];
  const activeMentionOptionId =
    hasMentionOptions && activeMentionOption
      ? `${listboxId}-option-${activeMentionOption.actorId}`
      : undefined;
  return (
    <div className={cx(TEAM_MESSAGE_COMPOSER_SHELL_CLASS, className)}>
      {contextText ? (
        <div className={TEAM_MESSAGE_COMPOSER_CONTEXT_CLASS}>{contextText}</div>
      ) : null}
      <div className={TEAM_MESSAGE_COMPOSER_EDITOR_ROW_CLASS}>
        <textarea
          id={id}
          name={name}
          ref={textareaRef}
          className={cx(TEAM_PANEL_TEXTAREA_CLASS, textareaClassName)}
          rows={rows}
          placeholder={placeholder}
          value={draft}
          aria-haspopup="listbox"
          aria-expanded={hasMentionOptions}
          aria-controls={hasMentionOptions ? listboxId : undefined}
          aria-activedescendant={activeMentionOptionId}
          onChange={onDraftChange}
          onClick={onTextareaClick}
          onKeyUp={onTextareaKeyUp}
          onBlur={onTextareaBlur}
          onCompositionStart={onCompositionStart}
          onCompositionEnd={onCompositionEnd}
          onKeyDown={onTextareaKeyDown}
        />
        <ActionButton
          tone="primary"
          size="sm"
          className={cx(TEAM_MESSAGE_COMPOSER_SEND_BUTTON_CLASS, sendButtonClassName)}
          onClick={onSend}
          disabled={disabled}
        >
          {busy && busyLabel ? busyLabel : sendLabel}
        </ActionButton>
      </div>
      {hasMentionOptions ? (
        <div className={TEAM_MESSAGE_COMPOSER_MENTION_MENU_CLASS}>
          <div className={TEAM_MESSAGE_COMPOSER_MENTION_HINT_CLASS}>
            Select teammate mention (`@` without selection stays plain text)
          </div>
          <div
            id={listboxId}
            role="listbox"
            className={TEAM_MESSAGE_COMPOSER_MENTION_LIST_CLASS}
          >
            {mentionOptions.map((option, index) => (
              <MenuOptionButton
                key={option.actorId}
                id={`${listboxId}-option-${option.actorId}`}
                role="option"
                aria-selected={index === activeMentionIndex}
                active={index === activeMentionIndex}
                data-team-mention-option={option.actorId}
                onMouseDown={(event) => {
                  event.preventDefault();
                  onSelectMention(option);
                }}
              >
                <span>{option.label}</span>
                <span className={TEAM_MESSAGE_COMPOSER_MENTION_ALIAS_CLASS}>
                  {`@${option.label}`}
                </span>
              </MenuOptionButton>
            ))}
          </div>
        </div>
      ) : null}
      <ToolbarRow className={TEAM_MESSAGE_COMPOSER_ACTIONS_ROW_CLASS}>
        <span className={TEAM_MESSAGE_COMPOSER_HELPER_TEXT_CLASS}>{helperText}</span>
      </ToolbarRow>
    </div>
  );
}
