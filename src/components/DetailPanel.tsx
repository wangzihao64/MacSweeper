import { useMemo } from "react";
import type { AppInfo, LeftoverItem } from "../api";
import { formatBytes, tildify } from "../utils";
import AppIcon from "./AppIcon";

interface DetailPanelProps {
  app: AppInfo | null;
  icon: string | null | undefined;
  appSize: number | undefined;
  leftovers: LeftoverItem[] | null;
  loading: boolean;
  unchecked: Set<string>;
  onToggle: (path: string) => void;
  onToggleAll: (checked: boolean) => void;
  onUninstall: (mode: "trash" | "delete") => void;
  onClose: () => void;
}

export default function DetailPanel({
  app,
  icon,
  appSize,
  leftovers,
  loading,
  unchecked,
  onToggle,
  onToggleAll,
  onUninstall,
  onClose,
}: DetailPanelProps) {
  const groups = useMemo(() => {
    const map = new Map<string, LeftoverItem[]>();
    for (const item of leftovers ?? []) {
      const list = map.get(item.kind) ?? [];
      list.push(item);
      map.set(item.kind, list);
    }
    return [...map.entries()];
  }, [leftovers]);

  const checked = (leftovers ?? []).filter((i) => !unchecked.has(i.path));
  const checkedBytes = checked.reduce((sum, i) => sum + i.size, 0);
  const leftoversBytes = (leftovers ?? []).reduce((sum, i) => sum + i.size, 0);
  const allChecked = (leftovers?.length ?? 0) > 0 && unchecked.size === 0;

  if (!app) {
    return (
      <aside className="detail">
        <div className="detail-empty" data-tauri-drag-region>
          <div className="detail-empty-mark">◐</div>
          <p>选择一个 App</p>
          <span>查看它的关联文件，然后安全卸载</span>
        </div>
      </aside>
    );
  }

  return (
    <aside className="detail">
      <header className="detail-header">
        <div className="detail-title">
          <AppIcon name={app.name} icon={icon} size={62} />
          <div className="detail-title-text">
            <h1 title={app.name}>{app.name}</h1>
            <div className="detail-sub">
              <span>{app.version ? `版本 ${app.version}` : "未知版本"}</span>
              {app.isSystem && <span className="badge">系统 App</span>}
            </div>
          </div>
          <button className="icon-btn" onClick={onClose} aria-label="关闭详情">
            ×
          </button>
        </div>
        <div className="detail-path" title={app.path}>
          {tildify(app.path)}
        </div>
        <div className="detail-stats">
          <div className="stat">
            <span>App 本体</span>
            <b>{appSize == null ? "计算中…" : formatBytes(appSize)}</b>
          </div>
          <div className="stat">
            <span>关联文件</span>
            <b>
              {loading
                ? "查找中…"
                : `${leftovers?.length ?? 0} 项 · ${formatBytes(leftoversBytes)}`}
            </b>
          </div>
        </div>
      </header>

      <div className="detail-scroll">
        <div className="section-head">
          <span>关联文件</span>
          {!loading && (leftovers?.length ?? 0) > 0 && (
            <button className="link" onClick={() => onToggleAll(!allChecked)}>
              {allChecked ? "全不选" : "全选"}
            </button>
          )}
        </div>

        {loading && (
          <div className="lo-skeleton">
            {Array.from({ length: 5 }).map((_, i) => (
              <div key={i} className="lo-skeleton-row" />
            ))}
          </div>
        )}

        {!loading && leftovers?.length === 0 && (
          <p className="muted">未发现关联文件，可以直接卸载。</p>
        )}

        {!loading &&
          groups.map(([kind, items]) => {
            const groupBytes = items.reduce((sum, i) => sum + i.size, 0);
            return (
              <section className="lo-group" key={kind}>
                <div className="lo-group-head">
                  <span>{kind}</span>
                  <span>
                    {items.length} 项 · {formatBytes(groupBytes)}
                  </span>
                </div>
                {items.map((item) => {
                  const isChecked = !unchecked.has(item.path);
                  return (
                    <label
                      key={item.path}
                      className={`lo-row ${isChecked ? "" : "lo-off"}`}
                    >
                      <input
                        type="checkbox"
                        checked={isChecked}
                        onChange={() => onToggle(item.path)}
                      />
                      <span className="lo-path" title={item.path}>
                        {tildify(item.path)}
                      </span>
                      <span className="lo-size">{formatBytes(item.size)}</span>
                    </label>
                  );
                })}
              </section>
            );
          })}

        {app.isSystem && (
          <div className="notice">
            系统 App 位于只读系统卷，受 macOS 完整性保护（SIP），无法卸载。
          </div>
        )}
      </div>

      <footer className="detail-footer">
        <div className="footer-info">
          <span>将清理 {checked.length} 项关联文件</span>
          <b>{formatBytes(checkedBytes + (appSize ?? 0))}</b>
        </div>
        <div className="footer-actions">
          <button
            className="btn btn-primary"
            disabled={app.isSystem}
            onClick={() => onUninstall("trash")}
          >
            移到废纸篓
          </button>
          <button
            className="btn btn-danger-outline"
            disabled={app.isSystem}
            onClick={() => onUninstall("delete")}
          >
            永久删除
          </button>
        </div>
      </footer>
    </aside>
  );
}
