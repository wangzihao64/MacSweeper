import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  type AppInfo,
  type LeftoverItem,
  type SkillInfo,
  type UninstallReport,
  type UpdateInfo,
} from "./api";
import { formatBytes } from "./utils";
import Sidebar, { type Filter, type View } from "./components/Sidebar";
import AppRow from "./components/AppRow";
import DetailPanel from "./components/DetailPanel";
import SkillRow from "./components/SkillRow";
import SkillPanel from "./components/SkillPanel";
import {
  ConfirmDialog,
  ErrorDialog,
  NoticeDialog,
  ReportDialog,
} from "./components/Dialogs";

type Dialog =
  | { kind: "confirm"; mode: "trash" | "delete" }
  | { kind: "report"; report: UninstallReport; appName: string }
  | { kind: "notice"; title: string; message: string }
  | { kind: "error"; message: string };

export default function App() {
  const [apps, setApps] = useState<AppInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [scanError, setScanError] = useState<string | null>(null);
  const [sizes, setSizes] = useState<Record<string, number>>({});
  const [icons, setIcons] = useState<Record<string, string | null>>({});
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<AppInfo | null>(null);
  const [leftovers, setLeftovers] = useState<LeftoverItem[] | null>(null);
  const [leftoversLoading, setLeftoversLoading] = useState(false);
  const [unchecked, setUnchecked] = useState<Set<string>>(new Set());
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [busy, setBusy] = useState(false);
  const [acknowledged, setAcknowledged] = useState(false);
  const [updates, setUpdates] = useState<Record<string, UpdateInfo>>({});
  const [updatesChecking, setUpdatesChecking] = useState(false);
  const [updatingPath, setUpdatingPath] = useState<string | null>(null);
  const [view, setView] = useState<View>("apps");
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [skillsLoading, setSkillsLoading] = useState(true);
  const [skillsError, setSkillsError] = useState<string | null>(null);
  const [selectedSkill, setSelectedSkill] = useState<SkillInfo | null>(null);
  const [updatingSkill, setUpdatingSkill] = useState<string | null>(null);

  const searchRef = useRef<HTMLInputElement>(null);
  const scanSeq = useRef(0);
  const skillSeq = useRef(0);
  const leftoverSeq = useRef(0);
  const updateSeq = useRef(0);
  const booted = useRef(false);

  const checkUpdatesFor = useCallback(async (list: AppInfo[]) => {
    const seq = ++updateSeq.current;
    setUpdatesChecking(true);
    try {
      const infos = await api.checkUpdates(
        list.map((a) => ({ path: a.path, bundleId: a.bundleId, version: a.version })),
      );
      if (seq !== updateSeq.current) return;
      const map: Record<string, UpdateInfo> = {};
      for (const info of infos) map[info.path] = info;
      setUpdates(map);
    } catch {
      // 网络或更新渠道不可用时忽略，不影响本体功能
    } finally {
      if (seq === updateSeq.current) setUpdatesChecking(false);
    }
  }, []);

  const hydrate = useCallback(async (list: AppInfo[], seq: number) => {
    api
      .appSizes(list.map((a) => a.path))
      .then((entries) => {
        if (seq !== scanSeq.current) return;
        const map: Record<string, number> = {};
        for (const [path, size] of entries) map[path] = size;
        setSizes(map);
      })
      .catch(() => {});

    const queue = [...list];
    const worker = async () => {
      for (;;) {
        const app = queue.shift();
        if (!app || seq !== scanSeq.current) return;
        try {
          const icon = await api.appIcon(app.path);
          if (seq !== scanSeq.current) return;
          setIcons((prev) => ({ ...prev, [app.path]: icon }));
        } catch {
          if (seq !== scanSeq.current) return;
          setIcons((prev) => ({ ...prev, [app.path]: null }));
        }
      }
    };
    await Promise.all(Array.from({ length: 4 }, worker));
  }, []);

  const loadSkills = useCallback(async () => {
    const seq = ++skillSeq.current;
    setSkillsLoading(true);
    setSkillsError(null);
    try {
      const list = await api.scanSkills();
      if (seq !== skillSeq.current) return;
      setSkills(list);
      setSelectedSkill((prev) =>
        prev ? list.find((s) => s.name === prev.name) ?? null : null,
      );
    } catch (err) {
      if (seq !== skillSeq.current) return;
      setSkillsError(String(err));
    } finally {
      if (seq === skillSeq.current) setSkillsLoading(false);
    }
  }, []);

  const scan = useCallback(async () => {
    const seq = ++scanSeq.current;
    setLoading(true);
    setScanError(null);
    setSelected(null);
    setLeftovers(null);
    setUnchecked(new Set());
    try {
      const list = await api.scanApps();
      if (seq !== scanSeq.current) return;
      setApps(list);
      setSizes({});
      setIcons({});
      setLoading(false);
      void hydrate(list, seq);
      void checkUpdatesFor(list);
    } catch (err) {
      if (seq !== scanSeq.current) return;
      setScanError(String(err));
      setLoading(false);
    }
  }, [hydrate, checkUpdatesFor]);

  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    void scan();
    void loadSkills();
  }, [scan, loadSkills]);

  const changeView = useCallback(
    (next: View) => {
      if (next === view) return;
      setView(next);
      setQuery("");
      setSelected(null);
      setLeftovers(null);
      setSelectedSkill(null);
    },
    [view],
  );

  const rescan = useCallback(() => {
    if (view === "skills") {
      void loadSkills();
    } else {
      void scan();
    }
  }, [view, loadSkills, scan]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      } else if (mod && e.key.toLowerCase() === "r") {
        e.preventDefault();
        if (!busy) rescan();
      } else if (e.key === "Escape") {
        if (dialog) {
          if (!busy) setDialog(null);
        } else if (selectedSkill) {
          setSelectedSkill(null);
        } else if (selected) {
          setSelected(null);
          setLeftovers(null);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rescan, dialog, busy, selected, selectedSkill]);

  const selectApp = useCallback(async (app: AppInfo) => {
    setSelected(app);
    setLeftovers(null);
    setUnchecked(new Set());
    setLeftoversLoading(true);
    const seq = ++leftoverSeq.current;
    try {
      const items = await api.findLeftovers(app.path, app.bundleId, [app.name]);
      if (seq !== leftoverSeq.current) return;
      setLeftovers(items);
    } catch {
      if (seq !== leftoverSeq.current) return;
      setLeftovers([]);
    } finally {
      if (seq === leftoverSeq.current) setLeftoversLoading(false);
    }
  }, []);

  const closeDetail = () => {
    setSelected(null);
    setLeftovers(null);
    setUnchecked(new Set());
  };

  const toggleLeftover = (path: string) => {
    setUnchecked((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  const toggleAllLeftovers = (checked: boolean) => {
    setUnchecked(checked ? new Set() : new Set((leftovers ?? []).map((i) => i.path)));
  };

  const checkedLeftovers = (leftovers ?? []).filter((i) => !unchecked.has(i.path));
  const checkedLeftoverBytes = checkedLeftovers.reduce((sum, i) => sum + i.size, 0);

  const counts = useMemo(
    () => ({
      all: apps.length,
      user: apps.filter((a) => !a.isSystem).length,
      system: apps.filter((a) => a.isSystem).length,
    }),
    [apps],
  );

  const updateCount = useMemo(
    () => Object.values(updates).filter((u) => u.updateAvailable).length,
    [updates],
  );

  const visibleApps = useMemo(() => {
    const q = query.trim().toLowerCase();
    return apps.filter((app) => {
      if (filter === "update" && !updates[app.path]?.updateAvailable) return false;
      if (filter === "user" && app.isSystem) return false;
      if (filter === "system" && !app.isSystem) return false;
      if (!q) return true;
      return (
        app.name.toLowerCase().includes(q) ||
        app.bundleId.toLowerCase().includes(q) ||
        app.path.toLowerCase().includes(q)
      );
    });
  }, [apps, filter, query, updates]);

  const totalBytes = useMemo(
    () => Object.values(sizes).reduce((sum, v) => sum + v, 0),
    [sizes],
  );
  const sizesReady = apps.length > 0 && Object.keys(sizes).length >= apps.length;

  const visibleSkills = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return skills;
    return skills.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q) ||
        (s.source ?? "").toLowerCase().includes(q),
    );
  }, [skills, query]);

  const managedSkillCount = useMemo(
    () => skills.filter((s) => s.managed).length,
    [skills],
  );

  const closeDialog = () => {
    if (busy) return;
    setDialog(null);
    setAcknowledged(false);
  };

  const confirmUninstall = async (mode: "trash" | "delete") => {
    if (!selected) return;
    setBusy(true);
    try {
      const report = await api.uninstall({
        appPath: selected.path,
        bundleId: selected.bundleId,
        extraPaths: checkedLeftovers.map((i) => i.path),
        mode,
      });
      setDialog({ kind: "report", report, appName: selected.name });
      setAcknowledged(false);
      setSelected(null);
      setLeftovers(null);
      void scan();
    } catch (err) {
      setDialog({ kind: "error", message: String(err) });
    } finally {
      setBusy(false);
    }
  };

  const handleUpdate = async (app: AppInfo) => {
    const info = updates[app.path];
    if (!info || updatingPath) return;
    setUpdatingPath(app.path);
    try {
      const message = await api.performUpdate(app.path, info.channel, info.target);
      setDialog({
        kind: "notice",
        title: info.channel === "homebrew" ? "Homebrew 升级" : "更新",
        message,
      });
      if (info.channel === "homebrew") void scan();
    } catch (err) {
      setDialog({ kind: "error", message: String(err) });
    } finally {
      setUpdatingPath(null);
    }
  };

  const handleSkillUpdate = async (skill: SkillInfo) => {
    if (updatingSkill) return;
    setUpdatingSkill(skill.name);
    try {
      const message = await api.updateSkill(skill.name);
      setDialog({ kind: "notice", title: "Skill 更新", message });
      await loadSkills();
    } catch (err) {
      setDialog({ kind: "error", message: String(err) });
    } finally {
      setUpdatingSkill(null);
    }
  };

  return (
    <div className="app">
      <Sidebar
        view={view}
        onView={changeView}
        filter={filter}
        onFilter={setFilter}
        counts={counts}
        updateCount={updateCount}
        skillCount={skills.length}
        scanning={view === "skills" ? skillsLoading : loading}
        onRescan={rescan}
      />

      <main className="main">
        <div className="toolbar" data-tauri-drag-region>
          <div className="search">
            <svg
              className="search-icon"
              width="14"
              height="14"
              viewBox="0 0 16 16"
              fill="none"
              aria-hidden="true"
            >
              <circle cx="7" cy="7" r="5" stroke="currentColor" strokeWidth="1.6" />
              <path d="M11 11l3 3" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
            </svg>
            <input
              ref={searchRef}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={
                view === "skills"
                  ? "搜索 Skill 名称、描述或来源"
                  : "搜索 App 名称或 Bundle ID"
              }
              spellCheck={false}
              autoComplete="off"
            />
            {query && (
              <button className="search-clear" onClick={() => setQuery("")} aria-label="清空搜索">
                ×
              </button>
            )}
          </div>
          <div className="toolbar-meta">
            {view === "skills"
              ? skillsLoading
                ? "正在扫描技能…"
                : `${visibleSkills.length} 个 Skill · ${managedSkillCount} 个可自动更新`
              : loading
                ? "正在扫描…"
                : `${visibleApps.length} 个 App · ${
                    sizesReady ? `共 ${formatBytes(totalBytes)}` : "计算大小中…"
                  }${
                    updatesChecking
                      ? " · 正在检查更新…"
                      : updateCount > 0
                        ? ` · ${updateCount} 个可更新`
                        : ""
                  }`}
          </div>
        </div>

        <div
          className="progress"
          hidden={view === "skills" ? !skillsLoading : !loading}
        >
          <span />
        </div>

        <div className="list">
          {view === "skills" ? (
            <>
              {skillsLoading && skills.length === 0 && <SkeletonRows />}

              {!skillsLoading && skillsError && (
                <div className="state-block">
                  <p>扫描技能失败</p>
                  <span>{skillsError}</span>
                  <button className="btn" onClick={() => void loadSkills()}>
                    重试
                  </button>
                </div>
              )}

              {!skillsLoading &&
                !skillsError &&
                visibleSkills.map((skill) => (
                  <SkillRow
                    key={skill.name}
                    skill={skill}
                    selected={selectedSkill?.name === skill.name}
                    onSelect={() => setSelectedSkill(skill)}
                  />
                ))}

              {!skillsLoading && !skillsError && visibleSkills.length === 0 && (
                <div className="state-block">
                  <p>没有找到匹配的 Skill</p>
                  <span>
                    {query ? "换个关键词试试" : "常见目录：~/.agents、~/.claude、~/.codex、~/.qoder"}
                  </span>
                </div>
              )}
            </>
          ) : (
            <>
              {loading && apps.length === 0 && <SkeletonRows />}

              {!loading && scanError && (
                <div className="state-block">
                  <p>扫描失败</p>
                  <span>{scanError}</span>
                  <button className="btn" onClick={() => void scan()}>
                    重试
                  </button>
                </div>
              )}

              {!loading &&
                !scanError &&
                visibleApps.map((app) => (
                  <AppRow
                    key={app.path}
                    app={app}
                    icon={icons[app.path]}
                    size={sizes[app.path]}
                    update={updates[app.path]}
                    selected={selected?.path === app.path}
                    onSelect={() => void selectApp(app)}
                  />
                ))}

              {!loading && !scanError && visibleApps.length === 0 && (
                <div className="state-block">
                  <p>没有找到匹配的 App</p>
                  <span>{query ? `换个关键词试试` : "当前筛选条件下为空"}</span>
                </div>
              )}
            </>
          )}
        </div>
      </main>

      {view === "skills" ? (
        <SkillPanel
          skill={selectedSkill}
          updating={updatingSkill === selectedSkill?.name}
          onUpdate={() => {
            if (selectedSkill) void handleSkillUpdate(selectedSkill);
          }}
          onClose={() => setSelectedSkill(null)}
        />
      ) : (
        <DetailPanel
          app={selected}
          icon={selected ? icons[selected.path] : undefined}
          appSize={selected ? sizes[selected.path] : undefined}
          leftovers={leftovers}
          loading={leftoversLoading}
          unchecked={unchecked}
          onToggle={toggleLeftover}
          onToggleAll={toggleAllLeftovers}
          onUninstall={(mode) => {
            setAcknowledged(false);
            setDialog({ kind: "confirm", mode });
          }}
          update={selected ? updates[selected.path] : undefined}
          updating={updatingPath === selected?.path}
          onUpdate={() => {
            if (selected) void handleUpdate(selected);
          }}
          onClose={closeDetail}
        />
      )}

      {dialog?.kind === "confirm" && selected && (
        <ConfirmDialog
          mode={dialog.mode}
          appName={selected.name}
          leftoverCount={checkedLeftovers.length}
          totalBytes={checkedLeftoverBytes + (sizes[selected.path] ?? 0)}
          busy={busy}
          acknowledged={acknowledged}
          onAcknowledged={setAcknowledged}
          onCancel={closeDialog}
          onConfirm={() => void confirmUninstall(dialog.mode)}
        />
      )}

      {dialog?.kind === "report" && (
        <ReportDialog
          report={dialog.report}
          appName={dialog.appName}
          onClose={closeDialog}
        />
      )}

      {dialog?.kind === "notice" && (
        <NoticeDialog
          title={dialog.title}
          message={dialog.message}
          onClose={closeDialog}
        />
      )}

      {dialog?.kind === "error" && (
        <ErrorDialog message={dialog.message} onClose={closeDialog} />
      )}
    </div>
  );
}

function SkeletonRows() {
  return (
    <div className="skeleton-rows">
      {Array.from({ length: 9 }).map((_, i) => (
        <div className="skeleton-row" key={i}>
          <span className="skeleton skeleton-icon" />
          <span className="skeleton-row-text">
            <span className="skeleton skeleton-line" style={{ width: `${30 + ((i * 17) % 40)}%` }} />
            <span className="skeleton skeleton-line skeleton-line-sm" style={{ width: `${45 + ((i * 13) % 30)}%` }} />
          </span>
          <span className="skeleton skeleton-size" />
        </div>
      ))}
    </div>
  );
}
