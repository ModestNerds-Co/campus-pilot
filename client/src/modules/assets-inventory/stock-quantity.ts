/** Exact quantity parsing and display helpers for immutable item scales. */

export const MAX_STOCK_QUANTITY_MINOR = Number.MAX_SAFE_INTEGER;

export function parseStockQuantity(value: string, scale: number, allowZero = false): number | null {
  if (!Number.isInteger(scale) || scale < 0 || scale > 6) return null;
  const normalized = value.trim();
  if (!/^\d+(\.\d*)?$/.test(normalized)) return null;
  const [whole, fraction = ""] = normalized.split(".");
  if (fraction.length > scale) return null;
  const parsed = Number(`${whole}${fraction.padEnd(scale, "0")}`);
  if (!Number.isSafeInteger(parsed) || parsed > MAX_STOCK_QUANTITY_MINOR) return null;
  return parsed > 0 || (allowZero && parsed === 0) ? parsed : null;
}

export function exactStockQuantity(valueMinor: number, scale: number): string {
  const value = String(Math.abs(Math.trunc(valueMinor))).padStart(scale + 1, "0");
  const sign = valueMinor < 0 ? "-" : "";
  if (scale === 0) return `${sign}${value}`;
  return `${sign}${value.slice(0, -scale)}.${value.slice(-scale)}`;
}

export function formatStockQuantity(valueMinor: number, scale: number): string {
  return exactStockQuantity(valueMinor, scale).replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, "");
}

export function quantityScaleLabel(scale: number): string {
  return scale === 0 ? "Whole units" : `${scale} decimal ${scale === 1 ? "place" : "places"}`;
}
