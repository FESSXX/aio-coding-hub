// Usage:
// - Used by `HomeRequestLogsPanel` to show the selected request log detail.
// - Keeps the dialog UI isolated from the main overview panel to reduce file size and improve cohesion.

import { cliBadgeTone, cliShortLabel } from "../../constants/clis";
import {
  useRequestAttemptLogsByTraceIdQuery,
  useRequestLogDetailQuery,
} from "../../query/requestLogs";
import type { RequestLogDetail } from "../../services/requestLogs";
import { Card } from "../../ui/Card";
import { Dialog } from "../../ui/Dialog";
import { cn } from "../../utils/cn";
import {
  computeOutputTokensPerSecond,
  formatDurationMs,
  formatTokensPerSecond,
  formatUsdRaw,
  sanitizeTtfbMs,
} from "../../utils/formatters";
import { ProviderChainView } from "../ProviderChainView";
import { resolveProviderLabel } from "../../pages/providers/baseUrl";
import { computeStatusBadge } from "./HomeLogShared";

export type RequestLogDetailDialogProps = {
  selectedLogId: number | null;
  onSelectLogId: (id: number | null) => void;
};

export function RequestLogDetailDialog({
  selectedLogId,
  onSelectLogId,
}: RequestLogDetailDialogProps) {
  const selectedLogQuery = useRequestLogDetailQuery(selectedLogId);
  const selectedLog = selectedLogQuery.data ?? null;
  const selectedLogLoading = selectedLogQuery.isFetching;

  const attemptLogsQuery = useRequestAttemptLogsByTraceIdQuery(selectedLog?.trace_id ?? null, 50);
  const attemptLogs = attemptLogsQuery.data ?? [];
  const attemptLogsLoading = attemptLogsQuery.isFetching;

  const finalProviderText = resolveProviderLabel(
    selectedLog?.final_provider_name,
    selectedLog?.final_provider_id
  );

  const statusBadge = selectedLog
    ? computeStatusBadge({
        status: selectedLog.status,
        errorCode: selectedLog.error_code,
        hasFailover: attemptLogs.length > 1,
      })
    : null;

  const hasTokens =
    selectedLog != null &&
    (selectedLog.input_tokens != null ||
      selectedLog.output_tokens != null ||
      selectedLog.total_tokens != null ||
      selectedLog.cache_read_input_tokens != null ||
      selectedLog.cache_creation_input_tokens != null ||
      selectedLog.cache_creation_5m_input_tokens != null ||
      selectedLog.cache_creation_1h_input_tokens != null ||
      selectedLog.cost_usd != null ||
      selectedLog.duration_ms != null ||
      selectedLog.ttfb_ms != null);
  return (
    <Dialog
      open={selectedLogId != null}
      onOpenChange={(open) => {
        if (!open) onSelectLogId(null);
      }}
      title="代理记录详情"
      description="先看关键指标，再看为什么会重试、跳过或切换供应商。"
      className="max-w-3xl"
    >
      {selectedLogLoading ? (
        <div className="text-sm text-slate-600 dark:text-slate-400">加载中…</div>
      ) : !selectedLog ? (
        <div className="text-sm text-slate-600 dark:text-slate-400">
          未找到记录详情（可能已过期被留存策略清理）。
        </div>
      ) : (
        <div className="space-y-3">
          {hasTokens ? (
            <Card padding="sm">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <div className="text-sm font-semibold text-slate-900 dark:text-slate-100">
                    关键指标
                  </div>
                  <div className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                    这次请求的输入输出、缓存、耗时与花费。
                  </div>
                </div>
                {statusBadge ? (
                  <span
                    className={cn("rounded-full px-2.5 py-1 text-xs font-medium", statusBadge.tone)}
                    title={statusBadge.title}
                  >
                    {statusBadge.text}
                  </span>
                ) : null}
              </div>

              <div className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
                <MetricCard label="输入 Token" value={selectedLog.input_tokens} />
                <MetricCard label="输出 Token" value={selectedLog.output_tokens} />
                <MetricCard label="缓存创建" value={resolveCacheWriteValue(selectedLog)} />
                <MetricCard label="缓存读取" value={selectedLog.cache_read_input_tokens} />
                <MetricCard label="总耗时" value={formatDurationMs(selectedLog.duration_ms)} />
                <MetricCard
                  label="TTFB"
                  value={(() => {
                    const ttfbMs = sanitizeTtfbMs(selectedLog.ttfb_ms, selectedLog.duration_ms);
                    return ttfbMs != null ? formatDurationMs(ttfbMs) : "—";
                  })()}
                />
                <MetricCard
                  label="速率"
                  value={(() => {
                    const rate = computeOutputTokensPerSecond(
                      selectedLog.output_tokens,
                      selectedLog.duration_ms,
                      sanitizeTtfbMs(selectedLog.ttfb_ms, selectedLog.duration_ms)
                    );
                    return rate != null ? formatTokensPerSecond(rate) : "—";
                  })()}
                />
                <MetricCard label="花费" value={formatUsdRaw(selectedLog.cost_usd)} />
              </div>
            </Card>
          ) : null}

          <Card padding="sm">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <div className="text-sm font-semibold text-slate-900 dark:text-slate-100">
                  决策链
                </div>
                <div className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                  用中文说明本次请求为何成功、失败、重试或切换供应商。
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-2 text-xs text-slate-500 dark:text-slate-400">
                <span
                  className={cn(
                    "rounded-full px-2 py-0.5 font-medium",
                    cliBadgeTone(selectedLog.cli_key)
                  )}
                >
                  {cliShortLabel(selectedLog.cli_key)}
                </span>
                <span className="rounded-full bg-slate-100 dark:bg-slate-700 px-2 py-0.5">
                  最终供应商：{finalProviderText || "未知"}
                </span>
              </div>
            </div>
            <ProviderChainView
              attemptLogs={attemptLogs}
              attemptLogsLoading={attemptLogsLoading}
              attemptsJson={selectedLog.attempts_json}
            />
          </Card>
        </div>
      )}
    </Dialog>
  );
}

function MetricCard({
  label,
  value,
}: {
  label: string;
  value: string | number | null | undefined;
}) {
  return (
    <div className="rounded-xl border border-slate-200/80 bg-slate-50/80 px-3 py-3 dark:border-slate-700 dark:bg-slate-800/70">
      <div className="text-xs text-slate-500 dark:text-slate-400">{label}</div>
      <div className="mt-1 text-lg font-semibold text-slate-900 dark:text-slate-100">
        {value == null || value === "" ? "—" : value}
      </div>
    </div>
  );
}

function resolveCacheWriteValue(selectedLog: RequestLogDetail) {
  if (
    selectedLog.cache_creation_5m_input_tokens != null &&
    selectedLog.cache_creation_5m_input_tokens > 0
  ) {
    return `${selectedLog.cache_creation_5m_input_tokens} (5m)`;
  }
  if (
    selectedLog.cache_creation_1h_input_tokens != null &&
    selectedLog.cache_creation_1h_input_tokens > 0
  ) {
    return `${selectedLog.cache_creation_1h_input_tokens} (1h)`;
  }
  if (selectedLog.cache_creation_input_tokens != null) {
    return selectedLog.cache_creation_input_tokens;
  }
  if (selectedLog.cache_creation_5m_input_tokens != null) {
    return `${selectedLog.cache_creation_5m_input_tokens} (5m)`;
  }
  if (selectedLog.cache_creation_1h_input_tokens != null) {
    return `${selectedLog.cache_creation_1h_input_tokens} (1h)`;
  }
  return "—";
}
