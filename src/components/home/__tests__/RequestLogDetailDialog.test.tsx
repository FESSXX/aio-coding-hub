import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RequestAttemptLog, RequestLogDetail } from "../../../services/requestLogs";
import { RequestLogDetailDialog } from "../RequestLogDetailDialog";

const requestLogQueryState = vi.hoisted(() => ({
  selectedLog: null as RequestLogDetail | null,
  selectedLogLoading: false,
  attemptLogs: [] as RequestAttemptLog[],
  attemptLogsLoading: false,
}));

vi.mock("../../../query/requestLogs", () => ({
  useRequestLogDetailQuery: () => ({
    data: requestLogQueryState.selectedLog,
    isFetching: requestLogQueryState.selectedLogLoading,
  }),
  useRequestAttemptLogsByTraceIdQuery: () => ({
    data: requestLogQueryState.attemptLogs,
    isFetching: requestLogQueryState.attemptLogsLoading,
  }),
}));

function createSelectedLog(overrides: Partial<RequestLogDetail> = {}): RequestLogDetail {
  return {
    id: 1,
    trace_id: "trace-1",
    cli_key: "claude",
    method: "post",
    path: "/v1/messages",
    query: "hello",
    excluded_from_stats: false,
    special_settings_json: null,
    status: 499,
    error_code: "GW_STREAM_ABORTED",
    duration_ms: 1234,
    ttfb_ms: 100,
    attempts_json: "[]",
    input_tokens: 10,
    output_tokens: 20,
    total_tokens: 30,
    cache_read_input_tokens: 5,
    cache_creation_input_tokens: 2,
    cache_creation_5m_input_tokens: 1,
    cache_creation_1h_input_tokens: null,
    usage_json: JSON.stringify({ input_tokens: 10, cache_creation_1h_input_tokens: 999 }),
    requested_model: "claude-3",
    final_provider_id: 12,
    final_provider_name: "Claude Bridge",
    final_provider_source_id: 7,
    final_provider_source_name: "OpenAI Primary",
    cost_usd: 0.12,
    cost_multiplier: 1.25,
    created_at_ms: null,
    created_at: 1000,
    ...overrides,
  };
}

function setRequestLogQueryState(overrides: Partial<typeof requestLogQueryState> = {}) {
  requestLogQueryState.selectedLog = overrides.selectedLog ?? null;
  requestLogQueryState.selectedLogLoading = overrides.selectedLogLoading ?? false;
  requestLogQueryState.attemptLogs = overrides.attemptLogs ?? [];
  requestLogQueryState.attemptLogsLoading = overrides.attemptLogsLoading ?? false;
}

describe("home/RequestLogDetailDialog", () => {
  it("renders loading state and closes via dialog close button", async () => {
    const onSelectLogId = vi.fn();
    setRequestLogQueryState({ selectedLogLoading: true });

    render(<RequestLogDetailDialog selectedLogId={1} onSelectLogId={onSelectLogId} />);

    expect(screen.getByText("加载中…")).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("关闭"));
    await waitFor(() => {
      expect(onSelectLogId).toHaveBeenCalledWith(null);
    });
  });

  it("renders metrics first and hides raw trace/query details", () => {
    setRequestLogQueryState({ selectedLog: createSelectedLog() });

    render(<RequestLogDetailDialog selectedLogId={1} onSelectLogId={vi.fn()} />);

    expect(screen.getByText("代理记录详情")).toBeInTheDocument();
    expect(screen.getByText("关键指标")).toBeInTheDocument();
    expect(screen.getByText("输入 Token")).toBeInTheDocument();
    expect(screen.getByText("输出 Token")).toBeInTheDocument();
    expect(screen.getByText("缓存创建")).toBeInTheDocument();
    expect(screen.getByText("缓存读取")).toBeInTheDocument();
    expect(screen.getByText("总耗时")).toBeInTheDocument();
    expect(screen.getByText("TTFB")).toBeInTheDocument();
    expect(screen.getByText("速率")).toBeInTheDocument();
    expect(screen.getByText("花费")).toBeInTheDocument();

    expect(screen.queryByText(/请求追踪 ID/)).not.toBeInTheDocument();
    expect(screen.queryByText(/查询参数/)).not.toBeInTheDocument();
    expect(screen.queryByText(/usage_json/)).not.toBeInTheDocument();
  });

  it("falls back to raw usage_json when JSON parsing fails without rendering raw json section", () => {
    setRequestLogQueryState({ selectedLog: createSelectedLog({ usage_json: "not-json" }) });

    render(<RequestLogDetailDialog selectedLogId={1} onSelectLogId={vi.fn()} />);

    expect(screen.queryByText("not-json")).not.toBeInTheDocument();
    expect(screen.getByText("关键指标")).toBeInTheDocument();
  });
});
