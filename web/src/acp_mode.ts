export function normalizeAcpModeId(modeId: string): string {
  const trimmed = modeId.trim();
  switch (trimmed) {
    case "yolo":
    case "yalo":
    case "danger_full_access":
    case "danger-full-access":
      return "full-access";
    default:
      return trimmed;
  }
}
