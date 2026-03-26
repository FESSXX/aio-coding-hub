import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Root } from "react-dom/client";
import type { ReactNode } from "react";

vi.mock("../App", () => ({
  default: () => <div data-testid="main-entry-app">mock app</div>,
}));

vi.mock("../components/AppErrorBoundary", () => ({
  AppErrorBoundary: ({ children }: { children: ReactNode }) => children,
}));

vi.mock("../services/frontendErrorReporter", async () => {
  const actual = await vi.importActual<typeof import("../services/frontendErrorReporter")>(
    "../services/frontendErrorReporter"
  );
  return {
    ...actual,
    installGlobalErrorReporting: vi.fn(),
  };
});

let appRoot: Root | null = null;

async function importMainEntry() {
  const mainModule = await import("../main");
  appRoot = mainModule.appRoot;
  await new Promise((resolve) => setTimeout(resolve, 0));
  return mainModule;
}

describe("main entry", () => {
  beforeEach(() => {
    vi.resetModules();
  });

  afterEach(async () => {
    appRoot?.unmount();
    appRoot = null;
    await new Promise((resolve) => setTimeout(resolve, 0));
  });

  it("renders without crashing", async () => {
    document.body.innerHTML = '<div id="root"></div>';

    await importMainEntry();

    expect(document.querySelector("[data-testid='main-entry-app']")).toBeInTheDocument();
  }, 30000);

  it("registers global frontend error handlers", async () => {
    document.body.innerHTML = '<div id="root"></div>';

    const reporter = await import("../services/frontendErrorReporter");
    await importMainEntry();

    expect(reporter.installGlobalErrorReporting).toHaveBeenCalled();
  }, 30000);
});
