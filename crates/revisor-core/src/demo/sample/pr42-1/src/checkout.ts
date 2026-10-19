import { CartItem, cartTotals } from "./cart";
import { formatCents } from "./lib/money";

export interface CheckoutRequest {
  items: CartItem[];
}

export function checkoutSummary(request: CheckoutRequest): string {
  const totals = cartTotals(request.items);
  return [
    `Subtotal: ${formatCents(totals.subtotal)}`,
    `Tax: ${formatCents(totals.tax)}`,
    `Total: ${formatCents(totals.total)}`,
  ].join("\n");
}
