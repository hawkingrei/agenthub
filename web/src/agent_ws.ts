export const AGENT_NOT_RUNNING_ERROR = "agent not running";

export function isAgentActiveStatus(status: string | null): boolean {
  return status === "running" || status === "idle";
}

export function shouldOpenAgentSocket(status: string | null): boolean {
  return isAgentActiveStatus(status);
}

export function shouldIgnoreAgentWsError(
  message: string,
  status: string | null
): boolean {
  if (message !== AGENT_NOT_RUNNING_ERROR) return false;
  return !isAgentActiveStatus(status);
}

export function sanitizeAgentError(
  error: string | null,
  status: string | null
): string | null {
  if (error === AGENT_NOT_RUNNING_ERROR && !isAgentActiveStatus(status)) {
    return null;
  }
  return error;
}
