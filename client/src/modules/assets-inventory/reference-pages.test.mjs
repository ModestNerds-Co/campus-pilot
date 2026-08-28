import assert from "node:assert/strict";
import test from "node:test";

import { MAX_STOCK_REFERENCE_RECORDS, loadAllStockReferences } from "./reference-pages.ts";

test("stock drawers load every active reference beyond the first API page", async () => {
  const requestedPages = [];
  const records = await loadAllStockReferences("Active items", async (page, perPage) => {
    requestedPages.push(page);
    const total = 251;
    const start = (page - 1) * perPage;
    return {
      records: Array.from({ length: Math.max(0, Math.min(perPage, total - start)) }, (_, offset) => start + offset),
      total,
      totalPages: 3,
    };
  });

  assert.equal(records.length, 251);
  assert.deepEqual(requestedPages.sort((left, right) => left - right), [1, 2, 3]);
  assert.equal(records.at(-1), 250);
});

test("stock drawers report an explicit safe-cap state", async () => {
  await assert.rejects(
    loadAllStockReferences("Active stores", async () => ({
      records: [],
      total: MAX_STOCK_REFERENCE_RECORDS + 1,
      totalPages: Math.ceil((MAX_STOCK_REFERENCE_RECORDS + 1) / 100),
    })),
    /exceed the 5,000-record drawer limit/,
  );
});
