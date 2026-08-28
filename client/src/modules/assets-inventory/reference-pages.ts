/** Bounded catalogue pagination for stock drawers that must not hide later records. */

export const MAX_STOCK_REFERENCE_RECORDS = 5_000;
const PAGE_SIZE = 100;
const PAGE_BATCH_SIZE = 4;

export interface ReferencePage<T> {
  records: T[];
  total: number;
  totalPages: number;
}

export async function loadAllStockReferences<T>(
  label: string,
  loadPage: (page: number, perPage: number) => Promise<ReferencePage<T>>,
): Promise<T[]> {
  const first = await loadPage(1, PAGE_SIZE);
  if (first.total > MAX_STOCK_REFERENCE_RECORDS || first.totalPages > MAX_STOCK_REFERENCE_RECORDS / PAGE_SIZE) {
    throw new Error(`${label} exceed the ${MAX_STOCK_REFERENCE_RECORDS.toLocaleString()}-record drawer limit. Inactivate records that are no longer in use, then retry.`);
  }

  const records = [...first.records];
  for (let start = 2; start <= first.totalPages; start += PAGE_BATCH_SIZE) {
    const pages = Array.from(
      { length: Math.min(PAGE_BATCH_SIZE, first.totalPages - start + 1) },
      (_, offset) => start + offset,
    );
    const loaded = await Promise.all(pages.map((page) => loadPage(page, PAGE_SIZE)));
    for (const result of loaded) records.push(...result.records);
  }

  return records;
}
