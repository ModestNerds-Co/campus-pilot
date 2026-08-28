import assert from "node:assert/strict";
import test from "node:test";

import { createIdempotencyKeyLifecycle } from "./create-idempotency-key.ts";

for (const recordType of ["item", "store"]) {
  test(`${recordType} create retries reuse the same idempotency key`, () => {
    let sequence = 0;
    const lifecycle = createIdempotencyKeyLifecycle(() => `${recordType}-${++sequence}`);

    const ambiguousFirstAttempt = lifecycle.current();
    const retry = lifecycle.current();

    assert.equal(retry, ambiguousFirstAttempt);
    assert.equal(sequence, 1);

    lifecycle.startFresh();
    assert.notEqual(lifecycle.current(), ambiguousFirstAttempt);
    assert.equal(sequence, 2);
  });
}
