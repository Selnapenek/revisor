import { Cents, sumCents } from "./money";
import { roundPrice } from "./legacy/priceUtils";

export interface CartItem {
  sku: string;
  name: string;
  unitPrice: Cents;
  quantity: number;
}

export interface CartTotals {
  subtotal: Cents;
  tax: Cents;
  total: Cents;
}

const TAX_RATE = 0.08;

export function cartTotals(items: CartItem[]): CartTotals {
  const subtotal = sumCents(items.map((item) => item.unitPrice * item.quantity));
  const tax = roundPrice(subtotal * TAX_RATE);
  return { subtotal, tax, total: subtotal + tax };
}
