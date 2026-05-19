import React, { act } from "react";
import type { Root } from "react-dom/client";
import { MantineProvider } from "@mantine/core";

export function installReactDomTestGlobals(): void {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

  if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
    window.matchMedia = ((query: string) =>
      ({
        matches: false,
        media: query,
        onchange: null,
        addListener: () => {},
        removeListener: () => {},
        addEventListener: () => {},
        removeEventListener: () => {},
        dispatchEvent: () => false,
      }) as MediaQueryList) as typeof window.matchMedia;
  }

  if (typeof globalThis.ResizeObserver !== "function") {
    globalThis.ResizeObserver = class ResizeObserver {
      observe(): void {}
      unobserve(): void {}
      disconnect(): void {}
    } as typeof ResizeObserver;
  }

  if (typeof document !== "undefined") {
    const documentWithFonts = document as Document & {
      fonts?: {
        addEventListener?: (type: string, listener: EventListenerOrEventListenerObject) => void;
        removeEventListener?: (type: string, listener: EventListenerOrEventListenerObject) => void;
      };
    };
    if (!documentWithFonts.fonts) {
      Object.defineProperty(documentWithFonts, "fonts", {
        configurable: true,
        value: {
          addEventListener: () => {},
          removeEventListener: () => {},
        },
      });
    } else {
      documentWithFonts.fonts.addEventListener ??= () => {};
      documentWithFonts.fonts.removeEventListener ??= () => {};
    }
  }
}

export function required<T>(value: T | null | undefined, message: string): T {
  if (value == null) {
    throw new Error(message);
  }
  return value;
}

export function renderWithMantine(root: Root, node: React.ReactNode): void {
  act(() => {
    root.render(<MantineProvider env="test">{node}</MantineProvider>);
  });
}
