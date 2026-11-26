import appIconUrl from "../assets/app-icon.png";

export type Filter = "all" | "update" | "user" | "system";
export type View = "apps" | "skills" | "history";

interface SidebarProps {
  view: View;
  onView: (view: View) => void;
  filter: Filter;
  onFilter: (filter: Filter) => void;
  counts: { all: number; user: number; system: number };
  updateCount: number;
  skillCount: number;
  historyCount: number;
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
  view,
  onView,
  filter,
  onFilter,
  counts,
  updateCount,
  skillCount,
  historyCount,
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
          <span>应用卸载与技能管理</span>
        </div>
      </div>

      <nav className="nav">
        <div className="nav-label">应用</div>
        {ITEMS.map((item) => (
          <button
            key={item.key}
            className={`nav-item ${view === "apps" && filter === item.key ? "active" : ""}`}
            onClick={() => {
              onView("apps");
              onFilter(item.key);
            }}
          >
            <span>{item.label}</span>
            <span className={`nav-count ${item.key === "update" && updateCount > 0 ? "nav-count-hot" : ""}`}>
              {countFor(item.key)}
            </span>
          </button>
        ))}

        <div className="nav-label nav-label-gap">技能</div>
        <button
          className={`nav-item ${view === "skills" ? "active" : ""}`}
          onClick={() => onView("skills")}
        >
          <span>已安装 Skill</span>
          <span className="nav-count">{skillCount}</span>
        </button>

        <div className="nav-label nav-label-gap">记录</div>
        <button
          className={`nav-item ${view === "history" ? "active" : ""}`}
          onClick={() => onView("history")}
        >
          <span>操作历史</span>
          <span className="nav-count">{historyCount}</span>
        </button>
      </nav>

      <div className="sidebar-footer">
        <div className="sidebar-hint">
          {view === "skills"
            ? scanning
              ? "正在扫描技能…"
              : `已发现 ${skillCount} 个 Skill`
            : view === "history"
              ? scanning
                ? "正在读取记录…"
                : `共 ${historyCount} 条操作记录`
              : scanning
                ? "正在扫描应用…"
                : `已索引 ${counts.all} 个 App，可卸载 ${counts.user} 个`}
        </div>
        <button className="btn btn-block" onClick={onRescan} disabled={scanning}>
          {scanning
            ? "处理中…"
            : view === "skills"
              ? "刷新技能"
              : view === "history"
                ? "刷新记录"
                : "重新扫描"}
        </button>
      </div>
    </aside>
  );
}
