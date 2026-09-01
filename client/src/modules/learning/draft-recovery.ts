const PREFIX = "campus-pilot:learning-recovery:v1";
const MAX_AGE_MS = 24 * 60 * 60 * 1000;

interface RecoveryRecord {
  body: string;
  savedAt: number;
}

function key(userId: string, kind: "submission" | "review", resourceId: string) {
  return `${PREFIX}:${userId}:${kind}:${resourceId}`;
}

export function readLearningRecovery(
  userId: string,
  kind: "submission" | "review",
  resourceId: string,
): RecoveryRecord | null {
  try {
    const storageKey = key(userId, kind, resourceId);
    const raw = window.sessionStorage.getItem(storageKey);
    if (!raw) return null;
    const value = JSON.parse(raw) as Partial<RecoveryRecord>;
    if (
      typeof value.body !== "string" ||
      typeof value.savedAt !== "number" ||
      Date.now() - value.savedAt > MAX_AGE_MS
    ) {
      window.sessionStorage.removeItem(storageKey);
      return null;
    }
    return { body: value.body, savedAt: value.savedAt };
  } catch {
    return null;
  }
}

export function writeLearningRecovery(
  userId: string,
  kind: "submission" | "review",
  resourceId: string,
  body: string,
) {
  try {
    window.sessionStorage.setItem(
      key(userId, kind, resourceId),
      JSON.stringify({ body, savedAt: Date.now() } satisfies RecoveryRecord),
    );
  } catch {
    // Server drafts remain authoritative when browser storage is unavailable.
  }
}

export function clearLearningRecovery(
  userId: string,
  kind: "submission" | "review",
  resourceId: string,
) {
  try {
    window.sessionStorage.removeItem(key(userId, kind, resourceId));
  } catch {
    // Storage may be unavailable in privacy modes.
  }
}

export function purgeLearningRecoveryForOtherUsers(userId: string) {
  try {
    for (let index = window.sessionStorage.length - 1; index >= 0; index -= 1) {
      const storageKey = window.sessionStorage.key(index);
      if (storageKey?.startsWith(`${PREFIX}:`) && !storageKey.startsWith(`${PREFIX}:${userId}:`)) {
        window.sessionStorage.removeItem(storageKey);
      }
    }
  } catch {
    // Storage may be unavailable in privacy modes.
  }
}
