type PlanSummary = {
  total: number;
  completed: number;
  active: number;
  pending: number;
  ratio: number;
};

export function summarizePlanEntries(
  entries: Array<{ status?: string }>
): PlanSummary {
  const total = entries.length;
  if (total === 0) {
    return { total: 0, completed: 0, active: 0, pending: 0, ratio: 0 };
  }
  let completed = 0;
  let active = 0;
  for (const entry of entries) {
    const status = normalizePlanEntryStatus(entry.status);
    if (status === "completed") completed += 1;
    else if (status === "active") active += 1;
  }
  const pending = Math.max(0, total - completed - active);
  return {
    total,
    completed,
    active,
    pending,
    ratio: Math.round((completed / total) * 100),
  };
}

function normalizePlanEntryStatus(status?: string): "completed" | "active" | "pending" {
  if (!status) return "pending";
  const normalized = status.trim().toLowerCase().replace(/[\s-]+/g, "_");
  if (normalized === "completed" || normalized === "done" || normalized === "finished") {
    return "completed";
  }
  if (normalized === "in_progress" || normalized === "running" || normalized === "active") {
    return "active";
  }
  return "pending";
}

export type { PlanSummary };
