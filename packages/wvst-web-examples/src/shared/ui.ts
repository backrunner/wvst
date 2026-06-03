export function element<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) {
    throw new Error(`Missing element #${id}`);
  }
  return node as T;
}

export function readText(id: string): string {
  return element<HTMLInputElement>(id).value.trim();
}

export function readOptionalText(id: string): string | undefined {
  const value = readText(id);
  return value.length === 0 ? undefined : value;
}

export function readInteger(id: string, fallback: number): number {
  const value = Number.parseInt(element<HTMLInputElement>(id).value, 10);
  return Number.isFinite(value) ? value : fallback;
}

export function readOptionalInteger(id: string): number | undefined {
  const raw = element<HTMLInputElement>(id).value.trim();
  if (raw.length === 0) {
    return undefined;
  }

  const value = Number.parseInt(raw, 10);
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(`#${id} must be a non-negative integer`);
  }

  return value;
}

export function setStatus(message: string, kind: "idle" | "ok" | "error" = "idle"): void {
  const status = element<HTMLDivElement>("status");
  status.textContent = message;
  status.className = `status ${kind === "idle" ? "" : kind}`;
}

export function showJson(id: string, value: unknown): void {
  element<HTMLPreElement>(id).textContent = JSON.stringify(value, null, 2);
}

export function setButton(id: string, enabled: boolean): void {
  element<HTMLButtonElement>(id).disabled = !enabled;
}

export function optionValue(selectId: string): string | undefined {
  const value = element<HTMLSelectElement>(selectId).value;
  return value.length === 0 ? undefined : value;
}

export function fillSelect(
  id: string,
  rows: readonly { value: string; label: string }[],
  placeholder = "Select",
): void {
  const select = element<HTMLSelectElement>(id);
  select.innerHTML = "";
  select.append(option("", placeholder));
  for (const row of rows) {
    select.append(option(row.value, row.label));
  }
}

export function renderMetrics(id: string, metrics: Record<string, unknown>): void {
  const container = element<HTMLDivElement>(id);
  container.innerHTML = "";
  for (const [name, value] of Object.entries(metrics)) {
    const row = document.createElement("div");
    row.className = "metric";
    const label = document.createElement("span");
    label.textContent = name;
    const data = document.createElement("strong");
    data.textContent = metricValue(value);
    row.append(label, data);
    container.append(row);
  }
}

function option(value: string, label: string): HTMLOptionElement {
  const item = document.createElement("option");
  item.value = value;
  item.textContent = label;
  return item;
}

function metricValue(value: unknown): string {
  if (typeof value === "number") {
    return Number.isInteger(value) ? String(value) : value.toFixed(2);
  }
  return String(value);
}
