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
};
