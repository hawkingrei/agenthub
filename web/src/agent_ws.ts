export const AGENT_NOT_RUNNING_ERROR = "agent not running";

export function shouldOpenAgentSocket(status: string | null): boolean {
  return status === "running";
}

export function shouldIgnoreAgentWsError(
  message: string,
  status: string | null
): boolean {
  if (message !== AGENT_NOT_RUNNING_ERROR) return false;
  return status !== "running";
}

export function sanitizeAgentError(
  error: string | null,
  status: string | null
): string | null {
  if (error === AGENT_NOT_RUNNING_ERROR && status !== "running") {
    return null;
  }
  return error;
}
