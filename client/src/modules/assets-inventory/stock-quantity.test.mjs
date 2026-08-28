import assert from "node:assert/strict";
import test from "node:test";

import { exactStockQuantity, formatStockQuantity, parseStockQuantity } from "./stock-quantity.ts";

test("stock quantities parse without floating-point writes", () => {
  assert.equal(parseStockQuantity("12.345", 3), 12345);
  assert.equal(parseStockQuantity("12.3456", 3), null);
  assert.equal(parseStockQuantity("0", 0), null);
  assert.equal(parseStockQuantity("0", 0, true), 0);
  assert.equal(parseStockQuantity("1.1", 0), null);
  assert.equal(parseStockQuantity("9007199254740992", 0), null);
});

test("stock quantities retain their exact immutable scale", () => {
  assert.equal(exactStockQuantity(12030, 3), "12.030");
  assert.equal(formatStockQuantity(12030, 3), "12.03");
  assert.equal(formatStockQuantity(-500, 2), "-5");
});
