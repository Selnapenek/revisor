import { describe, expect, it } from "vitest";
import { cartTotals } from "../src/cart";

const mug = { sku: "mug", name: "Mug", unitPrice: 1250, quantity: 2 };

describe("cartTotals", () => {
  it("adds 8% tax", () => {
    expect(cartTotals([mug])).toEqual({ subtotal: 2500, discount: 0, tax: 200, total: 2700 });
  });

  it("applies a percent coupon", () => {
    const coupon = { code: "SPRING10", kind: "percent", percent: 10, expiresOn: "2099-01-01" } as const;
    const totals = cartTotals([mug], coupon);
    expect(totals.discount).toBe(250);
    expect(totals.total).toBe(2450);
  });
});
