export function isImeComposing(
  currentRefState: boolean,
  nativeIsComposing: boolean,
  nativeKeyCode?: number
): boolean {
  return currentRefState || nativeIsComposing || nativeKeyCode === 229;
}
