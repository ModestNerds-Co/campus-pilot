//
//  campus-pilot
//  changeTracker.ts
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

/**
 * Change Tracking Utility for Efficient Partial Updates
 *
 * This utility provides a robust system for tracking field-level changes
 * and generating minimal update payloads that only include modified fields.
 */

export interface ChangeEvent<T = any> {
  field: keyof T;
  oldValue: any;
  newValue: any;
  timestamp: Date;
}

export interface UpdatePayload<T = any> {
  id: string | number;
  changes: Partial<T>;
  metadata: {
    changedFields: string[];
    changeCount: number;
    timestamp: Date;
  };
}

export class ChangeTracker<T extends Record<string, any> = any> {
  private originalData: T;
  private currentData: T;
  private changeHistory: ChangeEvent<T>[] = [];
  private isDirty: boolean = false;

  constructor(initialData: T) {
    this.originalData = this.deepClone(initialData);
    this.currentData = this.deepClone(initialData);
  }

  /**
   * Update a single field and track the change
   */
  updateField<K extends keyof T>(field: K, value: T[K]): boolean {
    const oldValue = this.currentData[field];

    // Don't track if value hasn't actually changed
    if (this.isEqual(oldValue, value)) {
      return false;
    }

    // Record the change
    const changeEvent: ChangeEvent<T> = {
      field,
      oldValue,
      newValue: value,
      timestamp: new Date(),
    };

    this.changeHistory.push(changeEvent);
    this.currentData[field] = value;
    this.isDirty = true;

    return true;
  }

  /**
   * Update multiple fields at once
   */
  updateFields(updates: Partial<T>): string[] {
    const changedFields: string[] = [];

    Object.entries(updates).forEach(([field, value]) => {
      if (this.updateField(field as keyof T, value)) {
        changedFields.push(field);
      }
    });

    return changedFields;
  }

  /**
   * Get only the changed fields from original data
   */
  getChanges(): Partial<T> {
    const changes: Partial<T> = {};

    Object.keys(this.currentData).forEach((key) => {
      const typedKey = key as keyof T;
      if (
        !this.isEqual(this.originalData[typedKey], this.currentData[typedKey])
      ) {
        changes[typedKey] = this.currentData[typedKey];
      }
    });

    return changes;
  }

  /**
   * Get list of field names that have changed
   */
  getChangedFields(): string[] {
    return Object.keys(this.getChanges());
  }

  /**
   * Generate update payload with only changed data
   */
  generateUpdatePayload(id: string | number): UpdatePayload<T> | null {
    const changes = this.getChanges();
    const changedFields = Object.keys(changes);

    if (changedFields.length === 0) {
      return null; // No changes to send
    }

    return {
      id,
      changes,
      metadata: {
        changedFields,
        changeCount: changedFields.length,
        timestamp: new Date(),
      },
    };
  }

  /**
   * Reset to original state
   */
  reset(): void {
    this.currentData = this.deepClone(this.originalData);
    this.changeHistory = [];
    this.isDirty = false;
  }

  /**
   * Accept current changes as new baseline
   */
  commit(): void {
    this.originalData = this.deepClone(this.currentData);
    this.changeHistory = [];
    this.isDirty = false;
  }

  /**
   * Revert a specific field to its original value
   */
  revertField<K extends keyof T>(field: K): boolean {
    if (this.isEqual(this.originalData[field], this.currentData[field])) {
      return false; // No change to revert
    }

    this.currentData[field] = this.originalData[field];

    // Remove from change history
    this.changeHistory = this.changeHistory.filter(
      (change) => change.field !== field,
    );

    // Update dirty status
    this.isDirty = this.getChangedFields().length > 0;

    return true;
  }

  /**
   * Get current data
   */
  getCurrentData(): T {
    return this.deepClone(this.currentData);
  }

  /**
   * Get original data
   */
  getOriginalData(): T {
    return this.deepClone(this.originalData);
  }

  /**
   * Check if there are unsaved changes
   */
  hasChanges(): boolean {
    return this.isDirty && this.getChangedFields().length > 0;
  }

  /**
   * Get change history
   */
  getChangeHistory(): ChangeEvent<T>[] {
    return [...this.changeHistory];
  }

  /**
   * Get summary of changes for logging
   */
  getChangeSummary(): {
    totalChanges: number;
    changedFields: string[];
    firstChange: Date | null;
    lastChange: Date | null;
  } {
    const changedFields = this.getChangedFields();
    const history = this.changeHistory;

    return {
      totalChanges: changedFields.length,
      changedFields,
      firstChange: history.length > 0 ? history[0].timestamp : null,
      lastChange:
        history.length > 0 ? history[history.length - 1].timestamp : null,
    };
  }

  /**
   * Deep clone utility
   */
  private deepClone<U>(obj: U): U {
    if (obj === null || typeof obj !== "object") {
      return obj;
    }

    if (obj instanceof Date) {
      return new Date(obj.getTime()) as U;
    }

    if (Array.isArray(obj)) {
      return obj.map((item) => this.deepClone(item)) as U;
    }

    const cloned = {} as U;
    Object.keys(obj).forEach((key) => {
      cloned[key as keyof U] = this.deepClone((obj as any)[key]);
    });

    return cloned;
  }

  /**
   * Deep equality check
   */
  private isEqual(a: any, b: any): boolean {
    if (a === b) return true;

    if (a == null || b == null) return a === b;

    if (typeof a !== typeof b) return false;

    if (typeof a !== "object") return a === b;

    if (Array.isArray(a) !== Array.isArray(b)) return false;

    if (Array.isArray(a)) {
      if (a.length !== b.length) return false;
      return a.every((item, index) => this.isEqual(item, b[index]));
    }

    const keysA = Object.keys(a);
    const keysB = Object.keys(b);

    if (keysA.length !== keysB.length) return false;

    return keysA.every(
      (key) => keysB.includes(key) && this.isEqual(a[key], b[key]),
    );
  }
}

/**
 * Factory function for creating change trackers
 */
export function createChangeTracker<T extends Record<string, any>>(
  initialData: T,
): ChangeTracker<T> {
  return new ChangeTracker(initialData);
}

/**
 * Hook-like utility for React components
 */
export function useChangeTracker<T extends Record<string, any>>(
  initialData: T,
) {
  const tracker = new ChangeTracker(initialData);

  return {
    updateField: tracker.updateField.bind(tracker),
    updateFields: tracker.updateFields.bind(tracker),
    getChanges: tracker.getChanges.bind(tracker),
    getChangedFields: tracker.getChangedFields.bind(tracker),
    generateUpdatePayload: tracker.generateUpdatePayload.bind(tracker),
    reset: tracker.reset.bind(tracker),
    commit: tracker.commit.bind(tracker),
    revertField: tracker.revertField.bind(tracker),
    getCurrentData: tracker.getCurrentData.bind(tracker),
    getOriginalData: tracker.getOriginalData.bind(tracker),
    hasChanges: tracker.hasChanges.bind(tracker),
    getChangeHistory: tracker.getChangeHistory.bind(tracker),
    getChangeSummary: tracker.getChangeSummary.bind(tracker),
  };
}

/**
 * Utility for batch processing multiple entities
 */
export class BatchChangeTracker<T extends Record<string, any> = any> {
  private trackers = new Map<string | number, ChangeTracker<T>>();

  /**
   * Add or update an entity tracker
   */
  track(id: string | number, data: T): ChangeTracker<T> {
    const tracker = new ChangeTracker(data);
    this.trackers.set(id, tracker);
    return tracker;
  }

  /**
   * Get tracker for specific entity
   */
  getTracker(id: string | number): ChangeTracker<T> | undefined {
    return this.trackers.get(id);
  }

  /**
   * Get all entities with changes
   */
  getDirtyTrackers(): Map<string | number, ChangeTracker<T>> {
    const dirty = new Map<string | number, ChangeTracker<T>>();

    this.trackers.forEach((tracker, id) => {
      if (tracker.hasChanges()) {
        dirty.set(id, tracker);
      }
    });

    return dirty;
  }

  /**
   * Generate batch update payload
   */
  generateBatchUpdatePayload(): UpdatePayload<T>[] {
    const updates: UpdatePayload<T>[] = [];

    this.trackers.forEach((tracker, id) => {
      const payload = tracker.generateUpdatePayload(id);
      if (payload) {
        updates.push(payload);
      }
    });

    return updates;
  }

  /**
   * Commit all changes
   */
  commitAll(): void {
    this.trackers.forEach((tracker) => tracker.commit());
  }

  /**
   * Reset all changes
   */
  resetAll(): void {
    this.trackers.forEach((tracker) => tracker.reset());
  }

  /**
   * Remove tracker for entity
   */
  untrack(id: string | number): boolean {
    return this.trackers.delete(id);
  }

  /**
   * Get summary of all changes
   */
  getBatchSummary(): {
    totalEntities: number;
    entitiesWithChanges: number;
    totalChanges: number;
  } {
    let entitiesWithChanges = 0;
    let totalChanges = 0;

    this.trackers.forEach((tracker) => {
      if (tracker.hasChanges()) {
        entitiesWithChanges++;
        totalChanges += tracker.getChangedFields().length;
      }
    });

    return {
      totalEntities: this.trackers.size,
      entitiesWithChanges,
      totalChanges,
    };
  }
}
