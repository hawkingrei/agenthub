import MarkdownIt from "markdown-it";

type HighlightRenderer = (code: string, lang: string) => string;
type HrefSanitizer = (href: string) => string | null;

export function createConfiguredMarkdownRenderer({
  highlight,
  sanitizeHref,
}: {
  highlight: HighlightRenderer;
  sanitizeHref: HrefSanitizer;
}): MarkdownIt {
  const renderer = new MarkdownIt({
    html: false,
    linkify: false,
    typographer: true,
    highlight,
  });

  renderer.validateLink = (href: string) => sanitizeHref(href) != null;
  renderer.normalizeLink = (href: string) => sanitizeHref(href) ?? "";

  const defaultLinkOpen =
    renderer.renderer.rules.link_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.link_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrSet("target", "_blank");
    tokens[idx].attrSet("rel", "noopener noreferrer");
    tokens[idx].attrJoin("class", "md-link");
    return defaultLinkOpen(tokens, idx, options, env, self);
  };

  const defaultParagraphOpen =
    renderer.renderer.rules.paragraph_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.paragraph_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrJoin("class", "md-paragraph");
    return defaultParagraphOpen(tokens, idx, options, env, self);
  };

  const defaultBlockquoteOpen =
    renderer.renderer.rules.blockquote_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.blockquote_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrJoin("class", "md-blockquote");
    return defaultBlockquoteOpen(tokens, idx, options, env, self);
  };

  const defaultBulletListOpen =
    renderer.renderer.rules.bullet_list_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.bullet_list_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrJoin("class", "md-list md-list-unordered");
    return defaultBulletListOpen(tokens, idx, options, env, self);
  };

  const defaultOrderedListOpen =
    renderer.renderer.rules.ordered_list_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.ordered_list_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrJoin("class", "md-list md-list-ordered");
    return defaultOrderedListOpen(tokens, idx, options, env, self);
  };

  const defaultListItemOpen =
    renderer.renderer.rules.list_item_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.list_item_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrJoin("class", "md-list-item");
    return defaultListItemOpen(tokens, idx, options, env, self);
  };

  const defaultHeadingOpen =
    renderer.renderer.rules.heading_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.heading_open = (tokens, idx, options, env, self) => {
    const tag = tokens[idx]?.tag || "h1";
    tokens[idx].attrJoin("class", `md-heading md-${tag}`);
    return defaultHeadingOpen(tokens, idx, options, env, self);
  };

  const defaultHr =
    renderer.renderer.rules.hr ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.hr = (tokens, idx, options, env, self) => {
    tokens[idx].attrJoin("class", "md-divider");
    return defaultHr(tokens, idx, options, env, self);
  };

  const defaultCodeInline =
    renderer.renderer.rules.code_inline ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.code_inline = (tokens, idx, options, env, self) => {
    tokens[idx].attrJoin("class", "md-inline-code");
    return defaultCodeInline(tokens, idx, options, env, self);
  };

  const defaultFence = renderer.renderer.rules.fence;
  renderer.renderer.rules.fence = (tokens, idx, options, env, self) => {
    const raw = defaultFence
      ? defaultFence(tokens, idx, options, env, self)
      : self.renderToken(tokens, idx, options);
    const language = tokens[idx]?.info?.trim().split(/\s+/)[0] ?? "";
    return decoratePreTag(raw, language);
  };

  const defaultTableOpen =
    renderer.renderer.rules.table_open ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  const defaultTableClose =
    renderer.renderer.rules.table_close ??
    ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
  renderer.renderer.rules.table_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrJoin("class", "md-table");
    return '<div class="md-table-wrap">' + defaultTableOpen(tokens, idx, options, env, self);
  };
  renderer.renderer.rules.table_close = (tokens, idx, options, env, self) =>
    defaultTableClose(tokens, idx, options, env, self) + "</div>";

  return renderer;
}

function escapeAttribute(value: string): string {
  return value.replace(/"/g, "&quot;");
}

function decoratePreTag(html: string, language: string): string {
  return html.replace(/<pre\b([^>]*)>/, (_match, attrs: string) => {
    const existingClassMatch = attrs.match(/\bclass="([^"]*)"/);
    const existingClassNames = existingClassMatch?.[1]?.trim() ?? "";
    const nextClassNames = existingClassNames
      ? mergeClassNames("md-code-block", existingClassNames)
      : "md-code-block";
    const attrsWithoutClass = attrs.replace(/\s*\bclass="[^"]*"/, "");
    const languageAttr = language
      ? ` data-language="${escapeAttribute(language)}"`
      : "";
    return `<pre class="${nextClassNames}"${languageAttr}${attrsWithoutClass}>`;
  });
}

function mergeClassNames(...values: string[]): string {
  return Array.from(
    new Set(
      values
        .flatMap((value) => value.split(/\s+/))
        .map((value) => value.trim())
        .filter((value) => value.length > 0)
    )
  ).join(" ");
}
