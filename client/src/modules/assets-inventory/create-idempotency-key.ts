/** Keeps a create request key stable until the user starts a fresh form lifecycle. */

export interface CreateIdempotencyKeyLifecycle {
  current(): string;
  startFresh(): void;
}

export function createIdempotencyKeyLifecycle(
  createKey: () => string = () => crypto.randomUUID(),
): CreateIdempotencyKeyLifecycle {
  let key: string | null = null;

  return {
    current() {
      key ??= createKey();
      return key;
    },
    startFresh() {
      key = null;
    },
  };
}
