import type { AcpPermissionOption } from "./api";

export function isRejectPermissionOption(option: AcpPermissionOption): boolean {
  return option.kind === "reject_once" || option.kind === "reject_always";
}

export function resolveAcpPermissionOptionLabel(option: AcpPermissionOption): string {
  switch (option.kind) {
    case "allow_once":
      return "Allow";
    case "allow_always":
      return "Don't ask again";
    case "reject_once":
      return "Deny";
    case "reject_always":
      return "Deny and don't ask again";
    default:
      return option.name;
  }
}
