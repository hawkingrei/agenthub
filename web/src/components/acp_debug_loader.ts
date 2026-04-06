let acpDebugModulePromise: Promise<typeof import("./acp_debug")> | null = null;

export function loadAcpDebugModule(): Promise<typeof import("./acp_debug")> {
  if (acpDebugModulePromise == null) {
    acpDebugModulePromise = import("./acp_debug").catch((error) => {
      acpDebugModulePromise = null;
      throw error;
    });
  }
  return acpDebugModulePromise;
}
