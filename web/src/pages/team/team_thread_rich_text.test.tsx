// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { TeamThreadRichText } from "./team_thread_rich_text";
import {
  installReactDomTestGlobals,
  renderWithMantine,
} from "../../test_utils/react_test_helpers";

installReactDomTestGlobals();

describe("TeamThreadRichText", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it("renders sanitized team markdown html", () => {
    act(() => {
      renderWithMantine(
        root,
        <TeamThreadRichText text={`<script>alert(1)</script>\n<img src=x onerror=alert(1)>\n[unsafe](javascript:alert(1))`} />
      );
    });

    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector('a[href^="javascript:"]')).toBeNull();
    expect(container.innerHTML).toContain("&lt;script&gt;alert(1)&lt;/script&gt;");
    expect(container.innerHTML).toContain("&lt;img src=x onerror=alert(1)&gt;");
  });
});
