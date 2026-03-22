/**
 * Notice（系统通知）模块 - 全局事件监听
 *
 * 用法：
 * - 在 `src/App.tsx` 启动时调用 `listenNoticeEvents()`（只需要注册一次）
 * - 权限请求由 Settings 页面负责；此监听器仅在已授权时发送通知
 */

import { listen } from "@tauri-apps/api/event";

import { logToConsole } from "./consoleLog";
import type { NoticeLevel } from "./notice";

export type NoticeEventPayload = {
  level: NoticeLevel;
  title: string;
  body: string;
};

export async function listenNoticeEvents(): Promise<() => void> {
  const { isPermissionGranted, sendNotification } = await import("@tauri-apps/plugin-notification");

  const unlisten = await listen<NoticeEventPayload>("notice:notify", async (event) => {
    const payload = event.payload;
    if (!payload) return;

    try {
      const permissionGranted = await isPermissionGranted();
      if (!permissionGranted) return;

      await sendNotification({ title: payload.title, body: payload.body });
    } catch (err) {
      logToConsole("error", "发送系统通知失败", {
        error: String(err),
        level: payload.level,
        title: payload.title,
      });
    }
  });

  return () => {
    unlisten();
  };
}
