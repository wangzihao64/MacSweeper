import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  bundleId: string;
  version: string;
  path: string;
  executable: string;
  isSystem: boolean;
}

export interface LeftoverItem {
  path: string;
  kind: string;
  size: number;
}

export interface FailedItem {
  path: string;
  error: string;
}

export interface UninstallReport {
  removed: string[];
  failed: FailedItem[];
  quitAttempted: boolean;
}

export type UpdateChannel = "appstore" | "homebrew" | "sparkle" | "self";

export interface UpdateInfo {
  path: string;
  channel: UpdateChannel;
  latestVersion: string | null;
  installedVersion: string | null;
  updateAvailable: boolean;
  target: string;
  note: string | null;
}

export interface SkillLocation {
  root: string;
  path: string;
}

export interface SkillInfo {
  name: string;
  description: string;
  version: string | null;
  path: string;
  realPath: string;
  locations: SkillLocation[];
  source: string | null;
  sourceUrl: string | null;
  installedAt: string | null;
  updatedAt: string | null;
  managed: boolean;
}

export const api = {
  scanApps: () => invoke<AppInfo[]>("scan_apps"),
  appSizes: (paths: string[]) =>
    invoke<[string, number][]>("app_sizes", { paths }),
  appIcon: (appPath: string) => invoke<string | null>("app_icon", { appPath }),
  findLeftovers: (appPath: string, bundleId: string, names: string[]) =>
    invoke<LeftoverItem[]>("find_leftovers", { appPath, bundleId, names }),
  uninstall: (args: {
    appPath: string;
    bundleId: string;
    extraPaths: string[];
    mode: "trash" | "delete";
  }) => invoke<UninstallReport>("uninstall", args),
  checkUpdates: (apps: { path: string; bundleId: string; version: string }[]) =>
    invoke<UpdateInfo[]>("check_updates", { apps }),
  performUpdate: (appPath: string, channel: UpdateChannel, target: string) =>
    invoke<string>("perform_update", { appPath, channel, target }),
  scanSkills: () => invoke<SkillInfo[]>("scan_skills"),
  updateSkill: (name: string) => invoke<string>("update_skill", { name }),
};
