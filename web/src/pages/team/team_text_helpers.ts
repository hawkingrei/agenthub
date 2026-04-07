export function escapeTeamHtml(input: string): string {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function isTeamImeComposing(
  currentRefState: boolean,
  nativeIsComposing: boolean,
  nativeKeyCode?: number
): boolean {
  return currentRefState || nativeIsComposing || nativeKeyCode === 229;
}
