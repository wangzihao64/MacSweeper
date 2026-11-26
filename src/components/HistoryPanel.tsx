import type { HistoryEntry } from "../api";
import { formatBytes, formatDateTime, tildify } from "../utils";
import { KIND_LABEL, historyGlyph, historyGradient, modeLabel } from "./HistoryRow";

interface HistoryPanelProps {
  entry: HistoryEntry | null;
  onClose: () => void;
}

export default function HistoryPanel({ entry, onClose }: HistoryPanelProps) {
  if (!entry) {
    return (
      <aside className="detail">
        <div className="detail-empty" data-tauri-drag-region>
          <div className="detail-empty-mark">◷</div>
          <p>选择一条记录</p>
          <span>查看这次操作的时间与明细</span>
        </div>
      </aside>
    );
  }

  const failed = entry.failed > 0;

  return (
    <aside className="detail">
      <header className="detail-header">
        <div className="detail-title">
          <span
            className="history-icon history-icon-lg"
            style={{ backgroundImage: historyGradient(entry.kind) }}
          >
            {historyGlyph(entry.kind)}
          </span>
          <div className="detail-title-text">
            <h1 title={entry.title}>{entry.title}</h1>
            <div className="detail-sub">
              <span>{KIND_LABEL[entry.kind] ?? "操作"}</span>
              <span className={`badge ${failed ? "badge-danger" : ""}`}>
                {failed ? "存在失败项" : "全部完成"}
              </span>
            </div>
          </div>
          <button className="icon-btn" onClick={onClose} aria-label="关闭详情">
            ×
          </button>
        </div>
        <div className="detail-path">{formatDateTime(entry.timestamp)}</div>
        <div className="detail-stats">
          <div className="stat">
            <span>操作方式</span>
            <b>{modeLabel(entry.mode) ?? "—"}</b>
          </div>
          <div className="stat">
            <span>释放空间</span>
            <b>{entry.freedBytes > 0 ? formatBytes(entry.freedBytes) : "—"}</b>
          </div>
          <div className="stat">
            <span>成功项</span>
            <b>{entry.removed}</b>
          </div>
          <div className="stat">
            <span>失败项</span>
            <b className={failed ? "stat-danger" : undefined}>{entry.failed}</b>
          </div>
        </div>
      </header>

      <div className="detail-scroll">
        <div className="section-head">
          <span>操作明细</span>
        </div>
        <section className="lo-group">
          {entry.items.map((item, index) => (
            <div
              className={`hist-item ${item.ok ? "" : "hist-item-failed"}`}
              key={`${item.path}-${item.name}-${index}`}
            >
              <span className="hist-mark">{item.ok ? "✓" : "×"}</span>
              <span className="hist-main">
                <span className="hist-name">{item.name || tildify(item.path)}</span>
                {item.path && (
                  <span className="hist-path" title={item.path}>
                    {tildify(item.path)}
                  </span>
                )}
                {item.message && <span className="hist-msg">{item.message}</span>}
              </span>
            </div>
          ))}
          {entry.items.length === 0 && <p className="muted">这次操作没有产生明细。</p>}
        </section>
        <p className="muted">记录只保存在本机，最多保留最近 200 条操作。</p>
      </div>
    </aside>
  );
}
