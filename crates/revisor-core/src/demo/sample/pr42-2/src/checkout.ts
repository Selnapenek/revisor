import { CartItem, cartTotals } from "./cart";
import { findCoupon } from "./coupons";
import { formatCents } from "./lib/money";

export interface CheckoutRequest {
  items: CartItem[];
  couponCode?: string;
}

export function checkoutSummary(request: CheckoutRequest): string {
  const coupon = request.couponCode ? findCoupon(request.couponCode) : undefined;
  const totals = cartTotals(request.items, coupon);
  const lines = [`Subtotal: ${formatCents(totals.subtotal)}`];
  if (coupon && totals.discount > 0) {
    lines.push(`Discount (${coupon.code}): -${formatCents(totals.discount)}`);
  }
  lines.push(`Tax: ${formatCents(totals.tax)}`, `Total: ${formatCents(totals.total)}`);
  return lines.join("\n");
}
