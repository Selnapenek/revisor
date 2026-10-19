export type Cents = number;

export function formatCents(amount: Cents, currency = "USD"): string {
  const sign = amount < 0 ? "-" : "";
  const format = new Intl.NumberFormat("en-US", { style: "currency", currency });
  return sign + format.format(Math.abs(amount) / 100);
}

export function sumCents(amounts: Cents[]): Cents {
  return amounts.reduce((total, amount) => total + amount, 0);
}
