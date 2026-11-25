interface AppIconProps {
  name: string;
  icon: string | null | undefined;
  size: number;
}

function hueFor(name: string): number {
  let hash = 0;
  for (const ch of name) {
    hash = (hash * 31 + ch.charCodeAt(0)) % 360;
  }
  return hash;
}

export default function AppIcon({ name, icon, size }: AppIconProps) {
  const radius = Math.max(6, Math.round(size * 0.23));
  const style = {
    width: size,
    height: size,
    borderRadius: radius,
  } as const;

  if (icon) {
    return (
      <span className="app-icon" style={style}>
        <img src={icon} alt="" draggable={false} />
      </span>
    );
  }

  const hue = hueFor(name);
  const letter = (name.trim()[0] ?? "?").toUpperCase();
  return (
    <span
      className="app-icon app-icon-fallback"
      style={{
        ...style,
        fontSize: Math.round(size * 0.42),
        background: `linear-gradient(145deg, hsl(${hue} 62% 58%), hsl(${(hue + 46) % 360} 68% 42%))`,
      }}
    >
      {letter}
    </span>
  );
}
