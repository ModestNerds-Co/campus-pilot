export function formatAttendanceValue(value: string) {
  return value
    .replace(/[_-]/g, " ")
    .replace(/^./, (letter) => letter.toUpperCase());
}

export function formatAttendancePeriod(value: string) {
  if (!value.startsWith("lesson:")) return formatAttendanceValue(value);

  const periodKey = value.slice("lesson:".length);
  const number = periodKey.match(/(\d+)$/)?.[1];
  return number
    ? `Lesson · Period ${number}`
    : `Lesson · ${formatAttendanceValue(periodKey)}`;
}
