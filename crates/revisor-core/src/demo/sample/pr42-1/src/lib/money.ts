export type Cents = number;

export function formatCents(amount: Cents, currency = "USD"): string {
  return new Intl.NumberFormat("en-US", { style: "currency", currency }).format(amount / 100);
}

export function sumCents(amounts: Cents[]): Cents {
  return amounts.reduce((total, amount) => total + amount, 0);
}

export function percentOf(amount: Cents, percent: number): Cents {
  return Math.round((amount * percent) / 100);
}
