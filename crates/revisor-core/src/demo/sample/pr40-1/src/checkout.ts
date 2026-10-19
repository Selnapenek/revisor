import { CartItem, cartTotals } from "./cart";
import { formatCents } from "./money";

export interface CheckoutRequest {
  items: CartItem[];
  currency?: string;
}

export function checkoutSummary(request: CheckoutRequest): string {
  const totals = cartTotals(request.items);
  const currency = request.currency ?? "USD";
  return [
    `Subtotal: ${formatCents(totals.subtotal, currency)}`,
    `Tax: ${formatCents(totals.tax, currency)}`,
    `Total: ${formatCents(totals.total, currency)}`,
  ].join("\n");
}
