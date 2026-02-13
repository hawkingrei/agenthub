import MarkdownIt from "markdown-it";
import hljs from "highlight.js";

export function escapeHtml(input: string): string {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function decodeHtmlEntities(input: string): string {
  const named: Record<string, string> = {
    amp: "&",
    colon: ":",
    tab: "\t",
    newline: "\n",
  };
  let out = input;
  for (let i = 0; i < 2; i += 1) {
    out = out.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);/g, (_m, body) => {
      const key = String(body).toLowerCase();
      if (key in named) return named[key];
      if (key.startsWith("#x")) {
        const code = Number.parseInt(key.slice(2), 16);
        if (Number.isNaN(code)) return "";
        return String.fromCharCode(code);
      }
      if (key.startsWith("#")) {
        const code = Number.parseInt(key.slice(1), 10);
        if (Number.isNaN(code)) return "";
        return String.fromCharCode(code);
      }
      return "";
    });
  }
  return out;
}

function sanitizeHref(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;
  const decoded = decodeHtmlEntities(trimmed);
  // eslint-disable-next-line no-control-regex
  const normalized = decoded.replace(/[\u0000-\u001F\u007F\s]+/g, "").toLowerCase();
  if (
    normalized.startsWith("http://") ||
    normalized.startsWith("https://") ||
    normalized.startsWith("mailto:") ||
    normalized.startsWith("tel:")
  ) {
    return trimmed;
  }
  if (
    normalized.startsWith("/") ||
    normalized.startsWith("./") ||
    normalized.startsWith("../") ||
    normalized.startsWith("#")
  ) {
    return trimmed;
  }
  return null;
}

export function renderMarkdown(input: string): string {
  const stripped = input.replace(/cite[^]+/g, "").replace(/[]/g, "");
  return getMarkdownRenderer().render(stripped);
}

let markdownRenderer: MarkdownIt | null = null;

function getMarkdownRenderer(): MarkdownIt {
  if (markdownRenderer) return markdownRenderer;
  const renderer = new MarkdownIt({
    html: false,
    linkify: false,
    typographer: true,
    highlight: (code: string, lang: string) => {
      const language = lang ? lang.trim().split(/\s+/)[0] : "";
      if (language && hljs.getLanguage(language)) {
        try {
          return `<pre class="hljs"><code>${hljs.highlight(code, { language }).value}</code></pre>`;
        } catch {
          // fallback to escaped text
        }
      }
      return `<pre class="hljs"><code>${escapeHtml(code)}</code></pre>`;
    },
  });
  renderer.validateLink = (href: string) => sanitizeHref(href) != null;
  renderer.normalizeLink = (href: string) => sanitizeHref(href) ?? "";
  const defaultLinkOpen =
    renderer.renderer.rules.link_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.link_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrSet("target", "_blank");
    tokens[idx].attrSet("rel", "noopener noreferrer");
    return defaultLinkOpen(tokens, idx, options, env, self);
  };
  markdownRenderer = renderer;
  return renderer;
}
