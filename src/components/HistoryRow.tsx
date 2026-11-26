import type { HistoryEntry, HistoryKind } from "../api";
import { formatBytes, formatDateTime } from "../utils";

export const KIND_LABEL: Record<HistoryKind, string> = {
  uninstall: "卸载",
  "batch-uninstall": "批量卸载",
  "skill-update": "Skill 更新",
  "app-update": "应用更新",
};

const KIND_GLYPH: Record<HistoryKind, string> = {
  uninstall: "卸",
  "batch-uninstall": "批",
  "skill-update": "技",
  "app-update": "更",
};

const KIND_HUE: Record<HistoryKind, [number, number]> = {
  uninstall: [8, 344],
  "batch-uninstall": [24, 352],
  "skill-update": [262, 296],
  "app-update": [206, 242],
};

export function historyGradient(kind: HistoryKind): string {
  const [from, to] = KIND_HUE[kind] ?? [220, 260];
  return `linear-gradient(135deg, hsl(${from} 58% 54%), hsl(${to} 62% 46%))`;
}

export function historyGlyph(kind: HistoryKind): string {
  return KIND_GLYPH[kind] ?? "记";
}

export function modeLabel(mode: string | null): string | null {
  if (mode === "trash") return "移到废纸篓";
  if (mode === "delete") return "永久删除";
  return null;
}

function summary(entry: HistoryEntry): string {
  const parts: string[] = [];
  const mode = modeLabel(entry.mode);
  if (mode) parts.push(mode);
  if (entry.kind === "skill-update" || entry.kind === "app-update") {
    const message = entry.items[0]?.message;
    if (message) parts.push(message.split("\n")[0]);
  } else {
    if (entry.freedBytes > 0) parts.push(`释放 ${formatBytes(entry.freedBytes)}`);
    if (entry.removed > 0) parts.push(`${entry.removed} 项`);
  }
  if (entry.failed > 0) parts.push(`${entry.failed} 项失败`);
  return parts.join(" · ");
}

interface HistoryRowProps {
  entry: HistoryEntry;
  selected: boolean;
  onSelect: () => void;
}

export default function HistoryRow({ entry, selected, onSelect }: HistoryRowProps) {
  const failed = entry.failed > 0;
  return (
    <button
      className={`row ${selected ? "selected" : ""}`}
      onClick={onSelect}
    >
      <span
        className="history-icon"
        style={{ backgroundImage: historyGradient(entry.kind) }}
      >
        {historyGlyph(entry.kind)}
      </span>
      <span className="row-main">
        <span className="row-name">
          {entry.title}
          <span className="badge">{KIND_LABEL[entry.kind] ?? "操作"}</span>
          {failed && <span className="badge badge-danger">有失败项</span>}
        </span>
        <span className="history-sub">{summary(entry) || "—"}</span>
      </span>
      <span className="row-time">{formatDateTime(entry.timestamp)}</span>
    </button>
  );
}
