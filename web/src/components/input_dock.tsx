import { UnstyledButton } from "@mantine/core";
import React from "react";
import {
  ACP_JUMP_BOTTOM_BUTTON_CLASS,
  INPUT_DOCK_HISTORY_BUTTON_CLASS,
  INPUT_DOCK_HISTORY_ITEM_CLASS,
  INPUT_DOCK_HISTORY_MENU_CLASS,
  INPUT_DOCK_INTERRUPT_BUTTON_CLASS,
  INPUT_DOCK_ROOT_CLASS,
  INPUT_DOCK_SEND_BUTTON_CLASS,
  INPUT_DOCK_TEXTAREA_CLASS,
  TEAM_MESSAGE_COMPOSER_ACTIONS_ROW_CLASS,
  TEAM_MESSAGE_COMPOSER_EDITOR_ROW_CLASS,
  TEAM_MESSAGE_COMPOSER_HELPER_TEXT_CLASS,
} from "../ui/tailwind_classes";
import { isImeComposing } from "../input_ime";
import type { AgentInputImage } from "../api";

export const ACP_INPUT_MAX_IMAGES = 4;
export const ACP_INPUT_MAX_IMAGE_BYTES = 5 * 1024 * 1024;
export const ACP_INPUT_MAX_TOTAL_IMAGE_BYTES = 10 * 1024 * 1024;
const ACP_INPUT_IMAGE_TYPES = new Set([
  "image/png",
  "image/jpeg",
  "image/webp",
  "image/gif",
]);

type InputDockProps = {
  input: string;
  images?: AgentInputImage[];
  enableImages?: boolean;
  historyCommands: string[];
  showInterrupt: boolean;
  canInterrupt: boolean;
  sendDisabled?: boolean;
  onHeightChange?: (height: number) => void;
  onInputChange: (value: string) => void;
  onImagesChange?: (images: AgentInputImage[]) => void;
  onSendInput: () => void;
  onInterrupt: () => void;
  onNavigateHistory: (direction: "up" | "down") => void;
  onSelectHistoryCommand: (value: string) => void;
  onJumpToBottom: () => void;
  showConversationJump: boolean;
  isComposingRef: React.MutableRefObject<boolean>;
};

function approximateBase64Bytes(data: string): number {
  const normalized = data.replace(/\s/g, "");
  if (!normalized) return 0;
  const padding = normalized.endsWith("==") ? 2 : normalized.endsWith("=") ? 1 : 0;
  return Math.max(0, Math.floor((normalized.length * 3) / 4) - padding);
}

export function validateInputImageFiles(
  existing: AgentInputImage[],
  files: File[]
): string | null {
  if (existing.length + files.length > ACP_INPUT_MAX_IMAGES) {
    return `Attach up to ${ACP_INPUT_MAX_IMAGES} images.`;
  }
  for (const file of files) {
    if (!ACP_INPUT_IMAGE_TYPES.has(file.type.toLowerCase())) {
      return "Use PNG, JPEG, WebP, or GIF images.";
    }
    if (file.size <= 0) return "Empty images cannot be attached.";
    if (file.size > ACP_INPUT_MAX_IMAGE_BYTES) {
      return "Each image must be 5 MiB or smaller.";
    }
  }
  const existingBytes = existing.reduce(
    (total, image) => total + approximateBase64Bytes(image.data),
    0
  );
  const nextBytes = files.reduce((total, file) => total + file.size, existingBytes);
  if (nextBytes > ACP_INPUT_MAX_TOTAL_IMAGE_BYTES) {
    return "Attached images must total 10 MiB or less.";
  }
  return null;
}

function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error(`Failed to read ${file.name || "image"}.`));
    reader.onload = () => {
      const result = typeof reader.result === "string" ? reader.result : "";
      const separator = result.indexOf(",");
      if (separator < 0) {
        reject(new Error(`Failed to encode ${file.name || "image"}.`));
        return;
      }
      resolve(result.slice(separator + 1));
    };
    reader.readAsDataURL(file);
  });
}

function createInputImageId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `image-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export async function buildInputImages(files: File[]): Promise<AgentInputImage[]> {
  return Promise.all(
    files.map(async (file) => ({
      id: createInputImageId(),
      file_name: file.name || "pasted-image",
      mime_type: file.type.toLowerCase(),
      data: await readFileAsBase64(file),
    }))
  );
}

type VerticalRect = {
  top: number;
  bottom: number;
};

export type InputHistoryNavigationContext = {
  key: string;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  value: string;
  selectionStart: number | null;
  selectionEnd: number | null;
  isComposing: boolean;
};

export type InputDockKeyAction =
  | { type: "none" }
  | { type: "close_history" }
  | { type: "send" }
  | { type: "navigate_history"; direction: "up" | "down" };

type InputDockOutsideCloseDocument = Pick<
  Document,
  "addEventListener" | "removeEventListener"
>;

export function shouldCloseHistoryFromPointerTarget(
  target: EventTarget | null,
  container: { contains(node: Node): boolean } | null
): boolean {
  if (!container) return false;
  if (typeof Node === "undefined") return false;
  if (!(target instanceof Node)) return false;
  return !container.contains(target);
}

export function bindHistoryOutsideClose(
  doc: InputDockOutsideCloseDocument,
  container: { contains(node: Node): boolean } | null,
  onClose: () => void
): () => void {
  const handlePointerDown = (event: Event) => {
    if (!shouldCloseHistoryFromPointerTarget(event.target, container)) return;
    onClose();
  };
  doc.addEventListener("mousedown", handlePointerDown);
  doc.addEventListener("touchstart", handlePointerDown);
  return () => {
    doc.removeEventListener("mousedown", handlePointerDown);
    doc.removeEventListener("touchstart", handlePointerDown);
  };
}

export function deriveInputHistoryNavigation(
  ctx: InputHistoryNavigationContext
): "up" | "down" | null {
  if (ctx.isComposing) return null;
  if (ctx.key !== "ArrowUp" && ctx.key !== "ArrowDown") return null;
  if (ctx.shiftKey || ctx.altKey || ctx.metaKey || ctx.ctrlKey) return null;

  const value = ctx.value;
  const hasNewline = value.includes("\n");
  const selectionStart = ctx.selectionStart ?? 0;
  const selectionEnd = ctx.selectionEnd ?? selectionStart;
  const atStart = selectionStart === 0 && selectionEnd === 0;
  const atEnd = selectionStart === value.length && selectionEnd === value.length;

  if (ctx.key === "ArrowUp" && (atStart || !hasNewline)) return "up";
  if (ctx.key === "ArrowDown" && (atEnd || !hasNewline)) return "down";
  return null;
}

export type InputDockKeyActionContext = {
  key: string;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  sendOnEnter?: boolean;
  showHistory: boolean;
  composing: boolean;
  value: string;
  selectionStart: number | null;
  selectionEnd: number | null;
};

const MOBILE_INPUT_BREAKPOINT = 720;
const INPUT_VIEWPORT_SAFE_MARGIN = 12;

type RuntimeWindowLike = Pick<Window, "innerWidth" | "addEventListener" | "removeEventListener">;

function resolveRuntimeWindow(): RuntimeWindowLike | null {
  if (typeof window === "undefined") return null;
  return window;
}

export function isMobileInputViewport(width: number): boolean {
  return width <= MOBILE_INPUT_BREAKPOINT;
}

export function deriveInputPlaceholder(isMobileViewport: boolean): string {
  if (isMobileViewport) {
    return "Type a message (tap Send; Enter = newline)";
  }
  return "Send input (Enter to send, Shift+Enter for newline)";
}

export function deriveInputHelperText(isMobileViewport: boolean): string {
  if (isMobileViewport) {
    return "Tap Send to submit · Enter adds a new line";
  }
  return "Enter to send · Shift+Enter for newline";
}

export function isInputRectOutsideViewport(
  rect: VerticalRect,
  viewportTop: number,
  viewportHeight: number,
  safeMargin: number = INPUT_VIEWPORT_SAFE_MARGIN
): boolean {
  if (!Number.isFinite(rect.top) || !Number.isFinite(rect.bottom)) return false;
  if (!Number.isFinite(viewportTop) || !Number.isFinite(viewportHeight)) return false;
  if (viewportHeight <= 0) return false;
  const visibleTop = viewportTop + safeMargin;
  const visibleBottom = viewportTop + viewportHeight - safeMargin;
  return rect.top < visibleTop || rect.bottom > visibleBottom;
}

export function deriveInputViewportOffset(
  rect: VerticalRect,
  viewportTop: number,
  viewportHeight: number,
  safeMargin: number = INPUT_VIEWPORT_SAFE_MARGIN
): number {
  if (!Number.isFinite(rect.top) || !Number.isFinite(rect.bottom)) return 0;
  if (!Number.isFinite(viewportTop) || !Number.isFinite(viewportHeight)) return 0;
  if (viewportHeight <= 0) return 0;
  const visibleBottom = viewportTop + viewportHeight - safeMargin;
  if (!Number.isFinite(visibleBottom)) return 0;
  const overflow = rect.bottom - visibleBottom;
  if (overflow <= 0) return 0;
  return Math.ceil(overflow);
}

function useMobileInputViewport(): boolean {
  const runtimeWindow = resolveRuntimeWindow();
  const [isMobileViewport, setIsMobileViewport] = React.useState(() =>
    runtimeWindow ? isMobileInputViewport(runtimeWindow.innerWidth) : false
  );

  React.useEffect(() => {
    if (!runtimeWindow) return;
    const syncViewport = () => {
      setIsMobileViewport((previous) => {
        const next = isMobileInputViewport(runtimeWindow.innerWidth);
        return previous === next ? previous : next;
      });
    };
    syncViewport();
    runtimeWindow.addEventListener("resize", syncViewport);
    return () => {
      runtimeWindow.removeEventListener("resize", syncViewport);
    };
  }, [runtimeWindow]);

  return isMobileViewport;
}

export function deriveInputDockKeyAction(
  ctx: InputDockKeyActionContext
): InputDockKeyAction {
  if (ctx.key === "Escape" && ctx.showHistory) {
    return { type: "close_history" };
  }
  if (
    ctx.key === "Enter" &&
    !ctx.shiftKey &&
    !ctx.composing &&
    (ctx.sendOnEnter ?? true)
  ) {
    return { type: "send" };
  }
  const direction = deriveInputHistoryNavigation({
    key: ctx.key,
    shiftKey: ctx.shiftKey,
    altKey: ctx.altKey,
    metaKey: ctx.metaKey,
    ctrlKey: ctx.ctrlKey,
    value: ctx.value,
    selectionStart: ctx.selectionStart,
    selectionEnd: ctx.selectionEnd,
    isComposing: ctx.composing,
  });
  if (direction) {
    return { type: "navigate_history", direction };
  }
  return { type: "none" };
}

export function InputDock({
  input,
  images = [],
  enableImages = false,
  historyCommands,
  showInterrupt,
  canInterrupt,
  sendDisabled = false,
  onHeightChange,
  onInputChange,
  onImagesChange,
  onSendInput,
  onInterrupt,
  onNavigateHistory,
  onSelectHistoryCommand,
  onJumpToBottom,
  showConversationJump,
  isComposingRef,
}: InputDockProps) {
  const textareaId = React.useId();
  const [showHistory, setShowHistory] = React.useState(false);
  const [inputFocused, setInputFocused] = React.useState(false);
  const historyContainerRef = React.useRef<HTMLDivElement | null>(null);
  const inputDockRef = React.useRef<HTMLDivElement | null>(null);
  const textareaRef = React.useRef<HTMLTextAreaElement | null>(null);
  const imageInputRef = React.useRef<HTMLInputElement | null>(null);
  const imagesRef = React.useRef(images);
  const mountedRef = React.useRef(true);
  const lastReportedHeightRef = React.useRef<number | null>(null);
  const [attachmentError, setAttachmentError] = React.useState<string | null>(null);
  const visibleHistory = historyCommands.slice(0, 12);
  const mobileInputViewport = useMobileInputViewport();
  const inputPlaceholder = deriveInputPlaceholder(mobileInputViewport);
  const inputHelperText = deriveInputHelperText(mobileInputViewport);
  const effectiveSendDisabled =
    sendDisabled || (input.trim().length === 0 && images.length === 0);

  React.useEffect(() => {
    imagesRef.current = images;
  }, [images]);

  React.useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const addImageFiles = React.useCallback(
    async (fileList: FileList | File[]) => {
      if (!enableImages || !onImagesChange) return;
      const files = Array.from(fileList).filter((file) => file.type.startsWith("image/"));
      if (files.length === 0) return;
      const validationError = validateInputImageFiles(imagesRef.current, files);
      if (validationError) {
        setAttachmentError(validationError);
        return;
      }
      try {
        const next = await buildInputImages(files);
        if (!mountedRef.current) return;
        const latestImages = imagesRef.current;
        const completionError = validateInputImageFiles(latestImages, files);
        if (completionError) {
          setAttachmentError(completionError);
          return;
        }
        const mergedImages = [...latestImages, ...next];
        imagesRef.current = mergedImages;
        onImagesChange(mergedImages);
        setAttachmentError(null);
      } catch (error) {
        setAttachmentError(error instanceof Error ? error.message : "Failed to read image.");
      }
    },
    [enableImages, onImagesChange]
  );

  const reportDockHeight = React.useCallback(() => {
    if (!onHeightChange) return;
    const dockElement = inputDockRef.current;
    const nextHeight = dockElement
      ? Math.max(0, Math.ceil(dockElement.getBoundingClientRect().height))
      : 0;
    if (lastReportedHeightRef.current === nextHeight) {
      return;
    }
    lastReportedHeightRef.current = nextHeight;
    onHeightChange(nextHeight);
  }, [onHeightChange]);

  React.useEffect(() => {
    lastReportedHeightRef.current = null;
  }, [onHeightChange]);

  const ensureInputVisible = React.useCallback(() => {
    if (!mobileInputViewport) return;
    if (typeof window === "undefined") return;
    const textarea = textareaRef.current;
    if (!textarea) return;
    const viewport = window.visualViewport;
    const viewportTop = viewport?.offsetTop ?? 0;
    const viewportHeight = viewport?.height ?? window.innerHeight;
    const rect = textarea.getBoundingClientRect();
    const nextOffset = deriveInputViewportOffset(
      { top: rect.top, bottom: rect.bottom },
      viewportTop,
      viewportHeight
    );
    if (nextOffset > 0) {
      textarea.scrollIntoView({ block: "nearest", inline: "nearest" });
      inputDockRef.current?.scrollIntoView({ block: "end", inline: "nearest" });
      return;
    }
    if (
      !isInputRectOutsideViewport(
        { top: rect.top, bottom: rect.bottom },
        viewportTop,
        viewportHeight
      )
    ) {
      return;
    }
    inputDockRef.current?.scrollIntoView({ block: "end", inline: "nearest" });
  }, [mobileInputViewport]);

  React.useEffect(() => {
    if (!inputFocused || !mobileInputViewport) return;
    if (typeof window === "undefined") return;
    let rafId: number | null = null;
    const run = () => {
      if (typeof window.requestAnimationFrame === "function") {
        if (rafId != null && typeof window.cancelAnimationFrame === "function") {
          window.cancelAnimationFrame(rafId);
        }
        rafId = window.requestAnimationFrame(() => {
          rafId = null;
          ensureInputVisible();
        });
        return;
      }
      ensureInputVisible();
    };
    run();
    const viewport = window.visualViewport;
    viewport?.addEventListener("resize", run);
    viewport?.addEventListener("scroll", run);
    window.addEventListener("resize", run);
    return () => {
      if (
        rafId != null &&
        typeof window.cancelAnimationFrame === "function"
      ) {
        window.cancelAnimationFrame(rafId);
      }
      viewport?.removeEventListener("resize", run);
      viewport?.removeEventListener("scroll", run);
      window.removeEventListener("resize", run);
    };
  }, [ensureInputVisible, inputFocused, mobileInputViewport]);

  React.useEffect(() => {
    if (!showHistory) return;
    if (typeof document === "undefined") return;
    return bindHistoryOutsideClose(document, historyContainerRef.current, () => {
      setShowHistory(false);
    });
  }, [showHistory]);

  React.useEffect(() => {
    if (!onHeightChange) return;
    reportDockHeight();
    const dockElement = inputDockRef.current;
    if (!dockElement || typeof ResizeObserver !== "function") {
      return;
    }
    const observer = new ResizeObserver(() => {
      reportDockHeight();
    });
    observer.observe(dockElement);
    return () => {
      observer.disconnect();
    };
  }, [onHeightChange, reportDockHeight]);

  return (
    <div
      className="input-dock-shell relative flex self-stretch flex-col gap-0.5"
      data-acp-input-dock="true"
      ref={inputDockRef}
    >
      {showConversationJump && (
        <UnstyledButton
          type="button"
          className={`${ACP_JUMP_BOTTOM_BUTTON_CLASS} bottom-[calc(100%+0.75rem)] right-0`}
          onClick={onJumpToBottom}
          title="Jump to bottom"
          aria-label="Jump to bottom"
        >
          <i className="bi bi-chevron-down" aria-hidden="true" />
        </UnstyledButton>
      )}
      <div className={INPUT_DOCK_ROOT_CLASS}>
        {enableImages && images.length > 0 ? (
          <div
            className="flex flex-wrap gap-2 border-b border-notion-border/70 px-2 pb-2"
            data-acp-input-images="true"
          >
            {images.map((image) => (
              <div
                key={image.id}
                className="group relative h-16 w-16 overflow-hidden rounded-lg border border-notion-border bg-notion-hover"
              >
                <img
                  src={`data:${image.mime_type};base64,${image.data}`}
                  alt={image.file_name}
                  className="h-full w-full object-cover"
                />
                <UnstyledButton
                  type="button"
                  className="absolute right-1 top-1 inline-flex h-5 w-5 items-center justify-center rounded-full bg-slate-950/75 text-xs text-white opacity-80 transition hover:opacity-100"
                  onClick={() => {
                    onImagesChange?.(images.filter((candidate) => candidate.id !== image.id));
                    setAttachmentError(null);
                  }}
                  aria-label={`Remove ${image.file_name}`}
                  title={`Remove ${image.file_name}`}
                >
                  <i className="bi bi-x" aria-hidden="true" />
                </UnstyledButton>
              </div>
            ))}
          </div>
        ) : null}
        <div
          className={`input-editor-row ${TEAM_MESSAGE_COMPOSER_EDITOR_ROW_CLASS}`}
          data-input-editor-row="true"
          onDragOver={(event) => {
            if (!enableImages) return;
            event.preventDefault();
          }}
          onDrop={(event) => {
            if (!enableImages) return;
            event.preventDefault();
            void addImageFiles(event.dataTransfer.files);
          }}
        >
          <textarea
            id={textareaId}
            name="acp_input"
            className={`${INPUT_DOCK_TEXTAREA_CLASS} flex-1`}
            ref={textareaRef}
            placeholder={inputPlaceholder}
            value={input}
            onFocus={() => {
              setInputFocused(true);
              ensureInputVisible();
            }}
            onBlur={() => {
              setInputFocused(false);
            }}
            onChange={(e) => {
              setShowHistory(false);
              onInputChange(e.target.value);
              ensureInputVisible();
            }}
            onPaste={(event) => {
              if (!enableImages) return;
              const imageFiles = Array.from(event.clipboardData.files).filter((file) =>
                file.type.startsWith("image/")
              );
              if (imageFiles.length === 0) return;
              if (!event.clipboardData.getData("text/plain")) {
                event.preventDefault();
              }
              void addImageFiles(imageFiles);
            }}
            onCompositionStart={() => {
              isComposingRef.current = true;
            }}
            onCompositionEnd={() => {
              isComposingRef.current = false;
            }}
            onKeyDown={(e) => {
              const nativeEvent = e.nativeEvent as KeyboardEvent;
              const composing = isImeComposing(
                isComposingRef.current,
                nativeEvent.isComposing === true,
                nativeEvent.keyCode
              );
              const target = e.currentTarget;
              const action = deriveInputDockKeyAction({
                key: e.key,
                shiftKey: e.shiftKey,
                altKey: e.altKey,
                metaKey: e.metaKey,
                ctrlKey: e.ctrlKey,
                sendOnEnter: !mobileInputViewport,
                showHistory,
                composing,
                value: target.value,
                selectionStart: target.selectionStart,
                selectionEnd: target.selectionEnd,
              });
              if (action.type === "close_history") {
                setShowHistory(false);
                return;
              }
              if (action.type === "send") {
                if (effectiveSendDisabled) {
                  return;
                }
                e.preventDefault();
                onSendInput();
                setShowHistory(false);
                return;
              }
              if (action.type === "navigate_history") {
                e.preventDefault();
                setShowHistory(false);
                onNavigateHistory(action.direction);
              }
            }}
            rows={2}
          />
          <UnstyledButton
            type="button"
            className={INPUT_DOCK_SEND_BUTTON_CLASS}
            onClick={onSendInput}
            disabled={effectiveSendDisabled}
            aria-label="Send input"
            title="Send input"
          >
            <i className="bi bi-arrow-up" aria-hidden="true" />
          </UnstyledButton>
        </div>
        <div
          className={`input-row ${TEAM_MESSAGE_COMPOSER_ACTIONS_ROW_CLASS}`}
          role="group"
          aria-label="Input actions"
          data-input-actions-row="true"
        >
          <div className="flex min-w-0 flex-wrap items-center gap-1.5">
            {enableImages ? (
              <>
                <input
                  ref={imageInputRef}
                  type="file"
                  accept="image/png,image/jpeg,image/webp,image/gif"
                  multiple
                  className="hidden"
                  onChange={(event) => {
                    if (event.target.files) {
                      void addImageFiles(event.target.files);
                    }
                    event.target.value = "";
                  }}
                />
                <UnstyledButton
                  type="button"
                  className={INPUT_DOCK_HISTORY_BUTTON_CLASS}
                  onClick={() => imageInputRef.current?.click()}
                  disabled={images.length >= ACP_INPUT_MAX_IMAGES}
                  title="Attach images"
                  aria-label="Attach images"
                >
                  <i className="bi bi-image text-[12px]" aria-hidden="true" />
                  <span>Images</span>
                  {images.length > 0 ? <span>{images.length}</span> : null}
                </UnstyledButton>
              </>
            ) : null}
            <span className={TEAM_MESSAGE_COMPOSER_HELPER_TEXT_CLASS}>
              {inputHelperText}{enableImages ? " · paste or drop images" : ""}
            </span>
            {historyCommands.length > 0 && (
              <div className="input-history relative" ref={historyContainerRef}>
                <UnstyledButton
                  type="button"
                  className={INPUT_DOCK_HISTORY_BUTTON_CLASS}
                  onClick={() => setShowHistory((prev) => !prev)}
                  title="Show sent command history"
                  aria-label="Show sent command history"
                  aria-expanded={showHistory}
                  aria-haspopup="menu"
                >
                  <i className="bi bi-clock-history text-[12px]" aria-hidden="true" />
                  <span>History</span>
                  <span className="inline-flex min-w-[1.25rem] items-center justify-center rounded-full bg-white px-1.5 py-0.5 text-[10px] font-semibold text-slate-500 shadow-sm">
                    {visibleHistory.length}
                  </span>
                  <span
                    aria-hidden="true"
                    className={`text-[10px] text-slate-400 transition-transform ${
                      showHistory ? "rotate-180" : ""
                    }`}
                  >
                    ▾
                  </span>
                </UnstyledButton>
                {showHistory && (
                  <div className={INPUT_DOCK_HISTORY_MENU_CLASS} role="menu" aria-label="Sent command history">
                    {visibleHistory.map((item, idx) => (
                      <UnstyledButton
                        key={`${idx}-${item}`}
                        type="button"
                        className={INPUT_DOCK_HISTORY_ITEM_CLASS}
                        title={item}
                        onClick={() => {
                          onSelectHistoryCommand(item);
                          setShowHistory(false);
                        }}
                      >
                        {item}
                      </UnstyledButton>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>
          {showInterrupt && (
            <UnstyledButton
              type="button"
              className={INPUT_DOCK_INTERRUPT_BUTTON_CLASS}
              onClick={onInterrupt}
              disabled={!canInterrupt}
              title="Interrupt current run"
              aria-label="Interrupt current run"
            >
              <i className="bi bi-stop-circle text-[12px]" aria-hidden="true" />
              <span>Interrupt</span>
            </UnstyledButton>
          )}
        </div>
        {attachmentError ? (
          <div className="px-2 pb-1 text-[11px] font-medium text-rose-600" role="alert">
            {attachmentError}
          </div>
        ) : null}
      </div>
    </div>
  );
}
