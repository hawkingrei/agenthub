export type SeqComparable = {
  event_id?: number | null;
  ts?: number | null;
};

const compareEventId = (
  left?: number | null,
  right?: number | null
): number | null => {
  const leftVal = typeof left === "number" ? left : null;
  const rightVal = typeof right === "number" ? right : null;
  if (leftVal == null && rightVal == null) return null;
  if (leftVal == null) return -1;
  if (rightVal == null) return 1;
  if (leftVal === rightVal) return 0;
  return leftVal < rightVal ? -1 : 1;
};

export function compareEventOrder(left: SeqComparable, right: SeqComparable): number {
  const eventOrder = compareEventId(left.event_id ?? null, right.event_id ?? null);
  if (eventOrder != null) return eventOrder;
  const leftTs = left.ts ?? null;
  const rightTs = right.ts ?? null;
  if (leftTs == null && rightTs == null) return 0;
  if (leftTs == null) return -1;
  if (rightTs == null) return 1;
  if (leftTs === rightTs) return 0;
  return leftTs < rightTs ? -1 : 1;
}
