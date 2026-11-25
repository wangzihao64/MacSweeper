import appIconUrl from "../assets/app-icon.png";

export type Filter = "all" | "update" | "user" | "system";

interface SidebarProps {
  filter: Filter;
  onFilter: (filter: Filter) => void;
  counts: { all: number; user: number; system: number };
  updateCount: number;
  scanning: boolean;
  onRescan: () => void;
}

const ITEMS: { key: Filter; label: string }[] = [
  { key: "all", label: "所有 App" },
  { key: "update", label: "可更新" },
  { key: "user", label: "用户 App" },
  { key: "system", label: "系统 App" },
];

export default function Sidebar({
  filter,
  onFilter,
  counts,
  updateCount,
  scanning,
  onRescan,
}: SidebarProps) {
  const countFor = (key: Filter) =>
    key === "update" ? updateCount : counts[key];
  return (
    <aside className="sidebar">
      <div className="sidebar-drag" data-tauri-drag-region />
      <div className="brand">
        <img src={appIconUrl} alt="" className="brand-icon" draggable={false} />
        <div className="brand-text">
          <b>MacSweeper</b>
          <span>应用卸载与清理</span>
        </div>
      </div>

      <nav className="nav">
        <div className="nav-label">应用</div>
        {ITEMS.map((item) => (
          <button
            key={item.key}
            className={`nav-item ${filter === item.key ? "active" : ""}`}
            onClick={() => onFilter(item.key)}
          >
            <span>{item.label}</span>
            <span className={`nav-count ${item.key === "update" && updateCount > 0 ? "nav-count-hot" : ""}`}>
              {countFor(item.key)}
            </span>
          </button>
        ))}
      </nav>

      <div className="sidebar-footer">
        <div className="sidebar-hint">
          {scanning
            ? "正在扫描应用…"
            : `已索引 ${counts.all} 个 App，可卸载 ${counts.user} 个`}
        </div>
        <button className="btn btn-block" onClick={onRescan} disabled={scanning}>
          {scanning ? "扫描中…" : "重新扫描"}
        </button>
      </div>
    </aside>
  );
}
