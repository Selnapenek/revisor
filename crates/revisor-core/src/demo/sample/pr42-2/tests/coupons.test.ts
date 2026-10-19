import { describe, expect, it } from "vitest";
import { discountFor, findCoupon } from "../src/coupons";

describe("findCoupon", () => {
  it("finds an active coupon", () => {
    expect(findCoupon("WELCOME5", new Date("2026-01-01"))?.kind).toBe("fixed");
  });

  it("ignores expired coupons", () => {
    expect(findCoupon("SPRING10", new Date("2026-07-01"))).toBeUndefined();
  });

  it("ignores unknown codes", () => {
    expect(findCoupon("NOPE")).toBeUndefined();
  });
});

describe("discountFor", () => {
  it("takes a percent of the subtotal", () => {
    const coupon = findCoupon("SPRING10", new Date("2026-03-01"))!;
    expect(discountFor(coupon, 2000)).toBe(200);
  });
});
