import type { SkillInfo } from "../api";
import { formatDate, tildify } from "../utils";
import { ROOT_LABEL, skillGradient } from "./SkillRow";

interface SkillPanelProps {
  skill: SkillInfo | null;
  updating: boolean;
  onUpdate: () => void;
  onClose: () => void;
}

export default function SkillPanel({
  skill,
  updating,
  onUpdate,
  onClose,
}: SkillPanelProps) {
  if (!skill) {
    return (
      <aside className="detail">
        <div className="detail-empty" data-tauri-drag-region>
          <div className="detail-empty-mark">✦</div>
          <p>选择一个 Skill</p>
          <span>查看它的来源，并更新到最新版本</span>
        </div>
      </aside>
    );
  }

  const roots = skill.locations
    .map((loc) => ROOT_LABEL[loc.root] ?? loc.root)
    .join(" · ");

  return (
    <aside className="detail">
      <header className="detail-header">
        <div className="detail-title">
          <span
            className="skill-icon skill-icon-lg"
            style={{ backgroundImage: skillGradient(skill.name) }}
          >
            ✦
          </span>
          <div className="detail-title-text">
            <h1 title={skill.name}>{skill.name}</h1>
            <div className="detail-sub">
              <span>{skill.version ? `版本 ${skill.version}` : "未标注版本"}</span>
              <span className="badge">{skill.managed ? "Skills CLI" : "外部安装"}</span>
            </div>
          </div>
          <button className="icon-btn" onClick={onClose} aria-label="关闭详情">
            ×
          </button>
        </div>
        <div className="detail-path" title={skill.realPath}>
          {tildify(skill.path)}
        </div>
        <div className="detail-stats">
          <div className="stat">
            <span>来源</span>
            <b className="stat-source" title={skill.sourceUrl ?? undefined}>
              {skill.source ?? "未登记"}
            </b>
          </div>
          <div className="stat">
            <span>安装位置</span>
            <b>{skill.locations.length} 处</b>
          </div>
          <div className="stat">
            <span>首次安装</span>
            <b>{formatDate(skill.installedAt)}</b>
          </div>
          <div className="stat">
            <span>最近更新</span>
            <b>{formatDate(skill.updatedAt)}</b>
          </div>
        </div>
      </header>

      <div className="detail-scroll">
        <div className="section-head">
          <span>Skill 说明</span>
        </div>
        <p className="skill-about">{skill.description || "这个 Skill 没有提供描述。"}</p>

        <div className="section-head">
          <span>安装位置</span>
        </div>
        <section className="lo-group">
          {skill.locations.map((loc) => (
            <div className="loc-row" key={loc.path}>
              <span className="badge loc-badge">{ROOT_LABEL[loc.root] ?? loc.root}</span>
              <span className="lo-path" title={loc.path}>
                {tildify(loc.path)}
              </span>
            </div>
          ))}
        </section>

        {!skill.managed && (
          <div className="notice">
            这个 Skill 不是通过 Skills CLI 安装的（{roots}），
            MacSweeper 不会改动它，请在安装它的工具里更新。
          </div>
        )}
      </div>

      <footer className="detail-footer">
        {skill.managed ? (
          <>
            <div className="footer-info">
              <span>更新源</span>
              <b className="footer-source" title={skill.sourceUrl ?? undefined}>
                {skill.source ?? "Skills CLI"}
              </b>
            </div>
            <button className="btn btn-primary btn-block" onClick={onUpdate} disabled={updating}>
              {updating ? (
                <span className="btn-busy">
                  <span className="spinner" />
                  正在更新…
                </span>
              ) : (
                "更新到最新版本"
              )}
            </button>
          </>
        ) : (
          <div className="footer-note">由外部工具管理，无法在此更新。</div>
        )}
      </footer>
    </aside>
  );
}
