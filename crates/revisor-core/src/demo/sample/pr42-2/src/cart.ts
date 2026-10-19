import { Cents, sumCents } from "./lib/money";
import { Coupon, discountFor } from "./coupons";

export interface CartItem {
  sku: string;
  name: string;
  unitPrice: Cents;
  quantity: number;
}

export interface CartTotals {
  subtotal: Cents;
  discount: Cents;
  tax: Cents;
  total: Cents;
}

const TAX_RATE = 0.08;

export function cartTotals(items: CartItem[], coupon?: Coupon): CartTotals {
  const subtotal = sumCents(items.map((item) => item.unitPrice * item.quantity));
  const discount = coupon ? discountFor(coupon, subtotal) : 0;
  const tax = subtotal * TAX_RATE;
  return { subtotal, discount, tax, total: subtotal - discount + tax };
}
