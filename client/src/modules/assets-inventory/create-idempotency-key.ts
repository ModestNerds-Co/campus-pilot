/** Keeps a create request key stable until the user starts a fresh form lifecycle. */

export interface CreateIdempotencyKeyLifecycle {
  current(): string;
  currentForFingerprint(fingerprint: string): string;
  startFresh(): void;
}

export function createIdempotencyKeyLifecycle(
  createKey: () => string = () => crypto.randomUUID(),
): CreateIdempotencyKeyLifecycle {
  let key: string | null = null;
  let fingerprint: string | null = null;

  return {
    current() {
      key ??= createKey();
      return key;
    },
    currentForFingerprint(nextFingerprint) {
      if (fingerprint !== null && fingerprint !== nextFingerprint) key = null;
      fingerprint = nextFingerprint;
      key ??= createKey();
      return key;
    },
    startFresh() {
      key = null;
      fingerprint = null;
    },
  };
}
