//! Usage: Windows WSL related Tauri commands.

use crate::app_state::{ensure_db_ready, DbInitState, GatewayState};
#[cfg(windows)]
use crate::db;
use crate::shared::mutex_ext::MutexExt;
use crate::{blocking, gateway, settings, wsl};
#[cfg(windows)]
use tauri::Emitter;
use tauri::Manager;

#[tauri::command]
pub(crate) async fn wsl_detect() -> wsl::WslDetection {
    blocking::run(
        "wsl_detect",
        move || -> crate::shared::error::AppResult<wsl::WslDetection> { Ok(wsl::detect()) },
    )
    .await
    .unwrap_or(wsl::WslDetection {
        detected: false,
        distros: Vec::new(),
    })
}

#[tauri::command]
pub(crate) async fn wsl_host_address_get() -> Option<String> {
    blocking::run(
        "wsl_host_address_get",
        move || -> crate::shared::error::AppResult<Option<String>> {
            Ok(wsl::host_ipv4_best_effort())
        },
    )
    .await
    .unwrap_or(None)
}

#[tauri::command]
pub(crate) async fn wsl_config_status_get(
    distros: Option<Vec<String>>,
) -> Vec<wsl::WslDistroConfigStatus> {
    blocking::run(
        "wsl_config_status_get",
        move || -> crate::shared::error::AppResult<Vec<wsl::WslDistroConfigStatus>> {
            let distros = match distros {
                Some(v) if v.is_empty() => return Ok(Vec::new()),
                Some(v) if !v.is_empty() => v,
                _ => {
                    let detection = wsl::detect();
                    if !detection.detected || detection.distros.is_empty() {
                        return Ok(Vec::new());
                    }
                    detection.distros
                }
            };

            Ok(wsl::get_config_status(&distros))
        },
    )
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub(crate) async fn wsl_configure_clients(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<wsl::WslConfigureReport, String> {
    if !cfg!(windows) {
        return Ok(wsl::WslConfigureReport {
            ok: false,
            message: "WSL configuration is only available on Windows".to_string(),
            distros: Vec::new(),
        });
    }

    let db = ensure_db_ready(app.clone(), db_state.inner()).await?;

    let cfg = blocking::run("wsl_configure_clients_read_settings", {
        let app = app.clone();
        move || -> crate::shared::error::AppResult<settings::AppSettings> {
            Ok(settings::read(&app).unwrap_or_default())
        }
    })
    .await?;

    if cfg.gateway_listen_mode == settings::GatewayListenMode::Localhost {
        return Ok(wsl::WslConfigureReport {
            ok: false,
            message: "监听模式为“仅本地(127.0.0.1)”时，WSL 无法访问网关。请先切换到：WSL 自动检测 / 局域网 / 自定义地址。".to_string(),
            distros: Vec::new(),
        });
    }

    let detection = wsl::detect();
    if !detection.detected || detection.distros.is_empty() {
        return Ok(wsl::WslConfigureReport {
            ok: false,
            message: "WSL not detected".to_string(),
            distros: Vec::new(),
        });
    }

    let preferred_port = cfg.preferred_port;
    let status = blocking::run("wsl_configure_clients_ensure_gateway", {
        let app = app.clone();
        let db = db.clone();
        move || {
            let state = app.state::<GatewayState>();
            let mut manager = state.0.lock_or_recover();
            manager.start(&app, db, Some(preferred_port))
        }
    })
    .await?;

    let port = status
        .port
        .ok_or_else(|| "gateway_start returned no port".to_string())?;

    let host = match cfg.gateway_listen_mode {
        settings::GatewayListenMode::Localhost => "127.0.0.1".to_string(),
        settings::GatewayListenMode::WslAuto | settings::GatewayListenMode::Lan => {
            wsl::resolve_wsl_host(&cfg)
        }
        settings::GatewayListenMode::Custom => {
            let parsed = match gateway::listen::parse_custom_listen_address(
                &cfg.gateway_custom_listen_address,
            ) {
                Ok(v) => v,
                Err(err) => {
                    return Ok(wsl::WslConfigureReport {
                        ok: false,
                        message: format!("自定义监听地址无效：{err}"),
                        distros: Vec::new(),
                    });
                }
            };
            if gateway::listen::is_wildcard_host(&parsed.host) {
                wsl::resolve_wsl_host(&cfg)
            } else {
                parsed.host
            }
        }
    };

    let proxy_origin = format!("http://{}", gateway::listen::format_host_port(&host, port));
    let distros = detection.distros;
    let targets = settings::WslTargetCli {
        claude: true,
        codex: true,
        gemini: true,
    };
    let report = blocking::run(
        "wsl_configure_clients",
        move || -> crate::shared::error::AppResult<wsl::WslConfigureReport> {
            Ok(wsl::configure_clients(&distros, &targets, &proxy_origin))
        },
    )
    .await?;

    Ok(report)
}

/// WSL startup auto-configure: detect WSL environment and configure all CLI clients.
/// If the current listen mode is localhost, automatically switch to wsl_auto and restart the gateway.
#[cfg(windows)]
pub(crate) async fn wsl_auto_configure_on_startup(
    app: &tauri::AppHandle,
    db: db::Db,
    mut listen_mode: settings::GatewayListenMode,
    gateway_port: Option<u16>,
) -> Result<(), String> {
    // 1. Detect WSL
    let detection = blocking::run(
        "wsl_startup_detect",
        || -> crate::shared::error::AppResult<wsl::WslDetection> { Ok(wsl::detect()) },
    )
    .await
    .map_err(|e| e.to_string())?;

    if !detection.detected || detection.distros.is_empty() {
        tracing::info!("WSL startup auto-configure: no WSL environment detected, skipping");
        return Ok(());
    }

    tracing::info!(
        distros = ?detection.distros,
        "WSL startup auto-configure: detected {} WSL distro(s)",
        detection.distros.len()
    );

    // 2. If listen mode is localhost, switch to wsl_auto and restart gateway
    if listen_mode == settings::GatewayListenMode::Localhost {
        tracing::info!(
            "WSL startup auto-configure: listen mode is localhost, switching to wsl_auto"
        );

        if let Err(err) = blocking::run("wsl_startup_switch_listen_mode", {
            let app = app.clone();
            move || -> crate::shared::error::AppResult<()> {
                let mut cfg = settings::read(&app).unwrap_or_default();
                cfg.gateway_listen_mode = settings::GatewayListenMode::WslAuto;
                settings::write(&app, &cfg)?;
                Ok(())
            }
        })
        .await
        {
            tracing::error!(
                "WSL startup auto-configure: switch listen mode failed: {}",
                err
            );
            let report = wsl::WslConfigureReport {
                ok: false,
                message: format!("自动切换监听模式失败：{err}"),
                distros: Vec::new(),
            };
            let _ = app.emit("wsl:auto_config_result", &report);
            return Err(format!("switch listen mode failed: {err}"));
        }

        listen_mode = settings::GatewayListenMode::WslAuto;
        tracing::info!("WSL startup auto-configure: listen mode switched to wsl_auto");

        // Restart gateway to apply new listen mode.
        // IMPORTANT: must stop tasks gracefully; otherwise restarting may leak background tasks.
        crate::app::cleanup::stop_gateway_best_effort(app).await;

        let status = match blocking::run("wsl_startup_restart_gateway", {
            let app = app.clone();
            let db = db.clone();
            let preferred_port = gateway_port;
            move || {
                let state = app.state::<GatewayState>();
                let mut manager = state.0.lock_or_recover();
                manager.start(&app, db, preferred_port)
            }
        })
        .await
        {
            Ok(status) => status,
            Err(err) => {
                tracing::error!(
                    "WSL startup auto-configure: gateway restart failed: {}",
                    err
                );
                let report = wsl::WslConfigureReport {
                    ok: false,
                    message: format!("重启网关失败：{err}"),
                    distros: Vec::new(),
                };
                let _ = app.emit("wsl:auto_config_result", &report);
                return Err(format!("gateway restart failed: {err}"));
            }
        };

        let _ = app.emit("gateway:status", &status);
        return do_wsl_auto_configure(app, &detection.distros, listen_mode, status.port).await;
    }

    // 3. Execute configuration with existing settings
    do_wsl_auto_configure(app, &detection.distros, listen_mode, gateway_port).await
}

#[cfg(windows)]
async fn do_wsl_auto_configure(
    app: &tauri::AppHandle,
    distros: &[String],
    listen_mode: settings::GatewayListenMode,
    gateway_port: Option<u16>,
) -> Result<(), String> {
    let port = match gateway_port {
        Some(p) => p,
        None => {
            let report = wsl::WslConfigureReport {
                ok: false,
                message: "gateway port unknown".to_string(),
                distros: Vec::new(),
            };
            let _ = app.emit("wsl:auto_config_result", &report);
            return Err(report.message);
        }
    };

    // Read current settings to resolve host address
    let cfg = blocking::run("wsl_startup_read_cfg", {
        let app = app.clone();
        move || -> crate::shared::error::AppResult<settings::AppSettings> {
            Ok(settings::read(&app).unwrap_or_default())
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    let host = match listen_mode {
        settings::GatewayListenMode::Localhost => "127.0.0.1".to_string(),
        settings::GatewayListenMode::WslAuto | settings::GatewayListenMode::Lan => {
            wsl::resolve_wsl_host(&cfg)
        }
        settings::GatewayListenMode::Custom => {
            let parsed =
                gateway::listen::parse_custom_listen_address(&cfg.gateway_custom_listen_address)
                    .map_err(|e| format!("invalid custom listen address: {}", e))?;

            if gateway::listen::is_wildcard_host(&parsed.host) {
                wsl::resolve_wsl_host(&cfg)
            } else {
                parsed.host
            }
        }
    };

    let proxy_origin = format!("http://{}", gateway::listen::format_host_port(&host, port));

    // Configure all CLI targets by default
    let targets = settings::WslTargetCli {
        claude: true,
        codex: true,
        gemini: true,
    };

    let distros_owned = distros.to_vec();
    let report = blocking::run(
        "wsl_startup_configure",
        move || -> crate::shared::error::AppResult<wsl::WslConfigureReport> {
            Ok(wsl::configure_clients(
                &distros_owned,
                &targets,
                &proxy_origin,
            ))
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    tracing::info!(
        ok = report.ok,
        message = %report.message,
        "WSL startup auto-configure completed"
    );

    let _ = app.emit("wsl:auto_config_result", &report);

    Ok(())
}
