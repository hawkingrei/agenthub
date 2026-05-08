import type { AcpPermissionOption } from "./api";

const ACP_PERMISSION_OPTION_LABELS: Record<string, string> = {
  allow_once: "Allow",
  allow_always: "Don't ask again",
  reject_once: "Deny",
  reject_always: "Deny and don't ask again",
};

export const isRejectPermissionOption = (option: AcpPermissionOption): boolean =>
  option.kind === "reject_once" || option.kind === "reject_always";

export const resolveAcpPermissionOptionLabel = (option: AcpPermissionOption): string =>
  ACP_PERMISSION_OPTION_LABELS[option.kind] ?? option.name;

export function resolveAcpPermissionDecisionText(
  options: AcpPermissionOption[],
  selectedOptionId: string | null
): string {
  if (!selectedOptionId) {
    return "Denied";
  }
  const option = options.find((candidate) => candidate.option_id === selectedOptionId);
  if (!option) {
    return "Approved";
  }
  const decision = isRejectPermissionOption(option) ? "Denied" : "Approved";
  return `${decision} · ${resolveAcpPermissionOptionLabel(option)}`;
}
