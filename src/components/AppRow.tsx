import type { AppInfo, UpdateInfo } from "../api";
import { formatBytes } from "../utils";
import AppIcon from "./AppIcon";

interface AppRowProps {
  app: AppInfo;
  icon: string | null | undefined;
  size: number | undefined;
  update: UpdateInfo | undefined;
  selected: boolean;
  checked: boolean;
  onSelect: () => void;
  onToggle: () => void;
}

export default function AppRow({
  app,
  icon,
  size,
  update,
  selected,
  checked,
  onSelect,
  onToggle,
}: AppRowProps) {
  return (
    <div
      className={`row ${selected ? "selected" : ""}`}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter") onSelect();
      }}
      role="button"
      tabIndex={0}
      title={app.path}
    >
      <input
        type="checkbox"
        className="row-check"
        checked={checked}
        disabled={app.isSystem}
        aria-label={`选择 ${app.name}`}
        title={app.isSystem ? "系统 App 无法卸载" : "选中以批量卸载"}
        onClick={(e) => e.stopPropagation()}
        onChange={onToggle}
      />
      <AppIcon name={app.name} icon={icon} size={38} />
      <span className="row-main">
        <span className="row-name">
          {app.name}
          {app.isSystem && <span className="badge">系统</span>}
          {update?.updateAvailable && (
            <span className="badge badge-update">可更新 {update.latestVersion}</span>
          )}
        </span>
        <span className="row-sub">{app.bundleId}</span>
      </span>
      <span className={`row-size ${size == null ? "row-size-pending" : ""}`}>
        {size == null ? "计算中…" : formatBytes(size)}
      </span>
    </div>
  );
}
