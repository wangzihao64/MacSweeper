import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  type AppInfo,
  type LeftoverItem,
  type UninstallReport,
} from "./api";
import { formatBytes } from "./utils";
import Sidebar, { type Filter } from "./components/Sidebar";
import AppRow from "./components/AppRow";
import DetailPanel from "./components/DetailPanel";
import { ConfirmDialog, ErrorDialog, ReportDialog } from "./components/Dialogs";

type Dialog =
  | { kind: "confirm"; mode: "trash" | "delete" }
  | { kind: "report"; report: UninstallReport; appName: string }
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

  const searchRef = useRef<HTMLInputElement>(null);
  const scanSeq = useRef(0);
  const leftoverSeq = useRef(0);
  const booted = useRef(false);

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
    } catch (err) {
      if (seq !== scanSeq.current) return;
      setScanError(String(err));
      setLoading(false);
    }
  }, [hydrate]);

  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    void scan();
  }, [scan]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      } else if (mod && e.key.toLowerCase() === "r") {
        e.preventDefault();
        if (!busy) void scan();
      } else if (e.key === "Escape") {
        if (dialog) {
          if (!busy) setDialog(null);
        } else if (selected) {
          setSelected(null);
          setLeftovers(null);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [scan, dialog, busy, selected]);

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

  const visibleApps = useMemo(() => {
    const q = query.trim().toLowerCase();
    return apps.filter((app) => {
      if (filter === "user" && app.isSystem) return false;
      if (filter === "system" && !app.isSystem) return false;
      if (!q) return true;
      return (
        app.name.toLowerCase().includes(q) ||
        app.bundleId.toLowerCase().includes(q) ||
        app.path.toLowerCase().includes(q)
      );
    });
  }, [apps, filter, query]);

  const totalBytes = useMemo(
    () => Object.values(sizes).reduce((sum, v) => sum + v, 0),
    [sizes],
  );
  const sizesReady = apps.length > 0 && Object.keys(sizes).length >= apps.length;

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

  return (
    <div className="app">
      <Sidebar
        filter={filter}
        onFilter={setFilter}
        counts={counts}
        scanning={loading}
        onRescan={() => void scan()}
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
              placeholder="搜索 App 名称或 Bundle ID"
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
            {loading
              ? "正在扫描…"
              : `${visibleApps.length} 个 App · ${
                  sizesReady ? `共 ${formatBytes(totalBytes)}` : "计算大小中…"
                }`}
          </div>
        </div>

        <div className="progress" hidden={!loading}>
          <span />
        </div>

        <div className="list">
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
        </div>
      </main>

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
        onClose={closeDetail}
      />

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
