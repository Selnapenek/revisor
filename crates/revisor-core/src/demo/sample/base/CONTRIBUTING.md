# Contributing

## Code rules

- Money is always an integer number of cents. Never keep fractional cents in a result.
- Validate and normalize user input at the edge (`src/checkout.ts`) before it reaches the cart.
- Every new module in `src/` has a test file in `tests/`.
- No `any` types.
