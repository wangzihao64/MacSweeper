import type { SkillInfo } from "../api";

export const ROOT_LABEL: Record<string, string> = {
  agents: "Agents",
  claude: "Claude Code",
  codex: "Codex",
  "codex-system": "Codex 内置",
  qoder: "Qoder",
};

export function skillHue(name: string): number {
  let hash = 0;
  for (const ch of name) hash = (hash * 31 + ch.charCodeAt(0)) % 360;
  return hash;
}

export function skillGradient(name: string): string {
  const hue = skillHue(name);
  return `linear-gradient(135deg, hsl(${hue} 62% 56%), hsl(${(hue + 28) % 360} 66% 46%))`;
}

interface SkillRowProps {
  skill: SkillInfo;
  selected: boolean;
  onSelect: () => void;
}

export default function SkillRow({ skill, selected, onSelect }: SkillRowProps) {
  return (
    <button
      className={`row ${selected ? "selected" : ""}`}
      onClick={onSelect}
      title={skill.path}
    >
      <span
        className="skill-icon"
        style={{ backgroundImage: skillGradient(skill.name) }}
      >
        ✦
      </span>
      <span className="row-main">
        <span className="row-name">
          {skill.name}
          {skill.version && <span className="badge">v{skill.version}</span>}
          {skill.managed && <span className="badge badge-update">CLI 可更新</span>}
        </span>
        <span className="skill-desc">{skill.description || "暂无描述"}</span>
      </span>
      <span className={`row-source ${skill.source ? "" : "row-source-muted"}`}>
        {skill.source ?? "外部安装"}
      </span>
    </button>
  );
}
