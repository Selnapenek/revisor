import { describe, expect, it } from "vitest";
import { cartTotals } from "../src/cart";

const mug = { sku: "mug", name: "Mug", unitPrice: 1250, quantity: 2 };

describe("cartTotals", () => {
  it("adds 8% tax", () => {
    expect(cartTotals([mug])).toEqual({ subtotal: 2500, tax: 200, total: 2700 });
  });
});
