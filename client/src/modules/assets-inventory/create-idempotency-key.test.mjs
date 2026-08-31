import assert from "node:assert/strict";
import test from "node:test";

import { createIdempotencyKeyLifecycle } from "./create-idempotency-key.ts";

for (const recordType of ["item", "store", "movement", "receipt allocation", "reversal"]) {
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

test("fingerprinted retries keep their key until the payload changes", () => {
  let sequence = 0;
  const lifecycle = createIdempotencyKeyLifecycle(() => `request-${++sequence}`);
  const submitted = JSON.stringify({ version: 2, quantity: 12 });

  const firstAttempt = lifecycle.currentForFingerprint(submitted);
  assert.equal(lifecycle.currentForFingerprint(submitted), firstAttempt);
  assert.equal(sequence, 1);

  const changedPayload = JSON.stringify({ version: 2, quantity: 10 });
  assert.notEqual(lifecycle.currentForFingerprint(changedPayload), firstAttempt);
  assert.equal(sequence, 2);
});
