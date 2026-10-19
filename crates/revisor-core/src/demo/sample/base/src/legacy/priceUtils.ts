// Deprecated: use the helpers in src/money.ts.
export function roundPrice(value: number): number {
  return Math.round(value);
}

export function addPrices(a: number, b: number): number {
  return roundPrice(a + b);
}
