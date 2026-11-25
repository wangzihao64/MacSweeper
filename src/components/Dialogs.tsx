import type { ReactNode } from "react";
import type { UninstallReport } from "../api";
import { formatBytes, tildify } from "../utils";

interface ModalProps {
  children: ReactNode;
  onClose: () => void;
  dismissable?: boolean;
}

function Modal({ children, onClose, dismissable = true }: ModalProps) {
  return (
    <div
      className="overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && dismissable) onClose();
      }}
    >
      <div className="modal" role="dialog" aria-modal="true">
        {children}
      </div>
    </div>
  );
}

interface ConfirmDialogProps {
  mode: "trash" | "delete";
  appName: string;
  leftoverCount: number;
  totalBytes: number;
  busy: boolean;
  acknowledged: boolean;
  onAcknowledged: (value: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void;
}

export function ConfirmDialog({
  mode,
  appName,
  leftoverCount,
  totalBytes,
  busy,
  acknowledged,
  onAcknowledged,
  onCancel,
  onConfirm,
}: ConfirmDialogProps) {
  const permanent = mode === "delete";
  return (
    <Modal onClose={onCancel} dismissable={!busy}>
      <h2>{permanent ? "永久删除？" : "移到废纸篓？"}</h2>
      <p className="modal-lede">
        将卸载 <b>{appName}</b>
        {leftoverCount > 0
          ? `，并清理 ${leftoverCount} 项关联文件。`
          : "。"}
      </p>
      <div className="summary-row">
        <span>预计释放空间</span>
        <b>{formatBytes(totalBytes)}</b>
      </div>
      {permanent ? (
        <>
          <div className="danger-box">
            永久删除不会经过废纸篓，被删除的 App 与残留文件将无法恢复。
          </div>
          <label className="ack">
            <input
              type="checkbox"
              checked={acknowledged}
              onChange={(e) => onAcknowledged(e.target.checked)}
              disabled={busy}
            />
            我已知晓此操作无法恢复
          </label>
        </>
      ) : (
        <p className="muted">删除的项目会进入废纸篓，随时可以恢复。</p>
      )}
      <footer className="modal-actions">
        <button className="btn" onClick={onCancel} disabled={busy}>
          取消
        </button>
        <button
          className={`btn ${permanent ? "btn-danger-solid" : "btn-primary"}`}
          onClick={onConfirm}
          disabled={busy || (permanent && !acknowledged)}
        >
          {busy ? (
            <span className="btn-busy">
              <span className="spinner" />
              正在删除…
            </span>
          ) : permanent ? (
            "永久删除"
          ) : (
            "移到废纸篓"
          )}
        </button>
      </footer>
    </Modal>
  );
}

interface ReportDialogProps {
  report: UninstallReport;
  appName: string;
  onClose: () => void;
}

export function ReportDialog({ report, appName, onClose }: ReportDialogProps) {
  const hasFailures = report.failed.length > 0;
  const anyRemoved = report.removed.length > 0;
  return (
    <Modal onClose={onClose}>
      <h2>{anyRemoved ? "卸载完成" : "未能删除"}</h2>
      <p className="modal-lede">
        「{appName}」：成功处理 {report.removed.length} 项
        {hasFailures ? `，${report.failed.length} 项失败。` : "。"}
      </p>
      {report.quitAttempted && (
        <p className="muted">已尝试退出正在运行的 App。</p>
      )}
      {hasFailures && (
        <div className="fail-list">
          {report.failed.map((item) => (
            <div className="fail-item" key={item.path}>
              <span className="fail-path" title={item.path}>
                {tildify(item.path)}
              </span>
              <span className="fail-error">{item.error}</span>
            </div>
          ))}
        </div>
      )}
      <footer className="modal-actions">
        <button className="btn btn-primary" onClick={onClose}>
          完成
        </button>
      </footer>
    </Modal>
  );
}

interface ErrorDialogProps {
  message: string;
  onClose: () => void;
}

export function ErrorDialog({ message, onClose }: ErrorDialogProps) {
  return (
    <Modal onClose={onClose}>
      <h2>操作失败</h2>
      <p className="modal-lede">{message}</p>
      <footer className="modal-actions">
        <button className="btn btn-primary" onClick={onClose}>
          知道了
        </button>
      </footer>
    </Modal>
  );
}
