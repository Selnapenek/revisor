import { Cents, percentOf } from "./lib/money";

export type Coupon =
  | { code: string; kind: "percent"; percent: number; expiresOn: string }
  | { code: string; kind: "fixed"; amount: Cents; expiresOn: string };

const COUPONS: Coupon[] = [
  { code: "SPRING10", kind: "percent", percent: 10, expiresOn: "2026-06-01" },
  { code: "WELCOME5", kind: "fixed", amount: 500, expiresOn: "2027-01-01" },
];

export function findCoupon(code: string, today = new Date()): Coupon | undefined {
  const coupon = COUPONS.find((c) => c.code === code);
  if (!coupon) return undefined;
  if (new Date(coupon.expiresOn) < today) return undefined;
  return coupon;
}

export function discountFor(coupon: Coupon, subtotal: Cents): Cents {
  if (coupon.kind === "percent") {
    return percentOf(subtotal, coupon.percent);
  }
  return coupon.amount;
}
