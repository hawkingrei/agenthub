export type SeqComparable = {
  seq?: string | null;
  ts?: number | null;
};

const UUID_V7_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const isNumericSeq = (value: string): boolean => /^[0-9]+$/.test(value);

const isUuidV7 = (value: string): boolean => UUID_V7_PATTERN.test(value);

const compareNumericStrings = (left: string, right: string): number => {
  if (left.length !== right.length) {
    return left.length < right.length ? -1 : 1;
  }
  if (left === right) return 0;
  return left < right ? -1 : 1;
};

export function compareSeqValue(
  left?: string | null,
  right?: string | null
): number | null {
  if (!left || !right) return null;
  const leftNumeric = isNumericSeq(left);
  const rightNumeric = isNumericSeq(right);
  if (leftNumeric && rightNumeric) {
    return compareNumericStrings(left, right);
  }
  const leftUuid = isUuidV7(left);
  const rightUuid = isUuidV7(right);
  if (leftUuid && rightUuid) {
    if (left === right) return 0;
    return left < right ? -1 : 1;
  }
  return null;
}

export function compareEventOrder(left: SeqComparable, right: SeqComparable): number {
  const seqOrder = compareSeqValue(left.seq ?? null, right.seq ?? null);
  if (seqOrder != null && seqOrder !== 0) return seqOrder;
  const leftTs = left.ts ?? null;
  const rightTs = right.ts ?? null;
  if (leftTs != null && rightTs != null && leftTs !== rightTs) {
    return leftTs < rightTs ? -1 : 1;
  }
  return 0;
}
