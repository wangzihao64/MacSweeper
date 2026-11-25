import type { AppInfo, UpdateInfo } from "../api";
import { formatBytes } from "../utils";
import AppIcon from "./AppIcon";

interface AppRowProps {
  app: AppInfo;
  icon: string | null | undefined;
  size: number | undefined;
  update: UpdateInfo | undefined;
  selected: boolean;
  onSelect: () => void;
}

export default function AppRow({
  app,
  icon,
  size,
  update,
  selected,
  onSelect,
}: AppRowProps) {
  return (
    <button
      className={`row ${selected ? "selected" : ""}`}
      onClick={onSelect}
      title={app.path}
    >
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
    </button>
  );
}
