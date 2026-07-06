//! gRPC service implementations backed by [`ControlState`].
//!
//! These are the unary control and query endpoints. High-rate streaming is
//! delivered over the WebSocket hub, not gRPC server streaming, so the
//! streaming services in the proto are the wire contract only and are not served
//! here.

use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::pb;
use crate::state::ControlState;

/// The exact phrase required to switch into live trading, modeling the UI
/// confirmation the specification mandates.
pub const LIVE_CONFIRMATION: &str = "ENABLE LIVE TRADING";

fn ack_ok(message: impl Into<String>) -> pb::Ack {
    pb::Ack {
        ok: true,
        message: message.into(),
    }
}

fn ack_err(message: impl Into<String>) -> pb::Ack {
    pb::Ack {
        ok: false,
        message: message.into(),
    }
}

/// EngineControl service: start/stop, mode switching, emergency stop.
pub struct EngineControlSvc {
    state: Arc<ControlState>,
}

impl EngineControlSvc {
    /// Builds the service over shared state.
    #[must_use]
    pub fn new(state: Arc<ControlState>) -> EngineControlSvc {
        EngineControlSvc { state }
    }
}

#[tonic::async_trait]
impl pb::engine_control_server::EngineControl for EngineControlSvc {
    async fn start(&self, _req: Request<pb::StartRequest>) -> Result<Response<pb::Ack>, Status> {
        self.state.set_running(true);
        Ok(Response::new(ack_ok("engine started")))
    }

    async fn stop(&self, _req: Request<pb::StopRequest>) -> Result<Response<pb::Ack>, Status> {
        self.state.set_running(false);
        Ok(Response::new(ack_ok("engine stopped")))
    }

    async fn set_mode(
        &self,
        req: Request<pb::SetModeRequest>,
    ) -> Result<Response<pb::Ack>, Status> {
        let req = req.into_inner();
        let ack = match req.mode() {
            pb::EngineMode::Live => {
                if req.confirmation_phrase != LIVE_CONFIRMATION {
                    ack_err("live mode requires the exact confirmation phrase")
                } else if self.state.set_mode(pb::EngineMode::Live) {
                    ack_ok("live mode enabled")
                } else {
                    ack_err("cannot enable live mode while emergency stop is engaged")
                }
            }
            pb::EngineMode::DryRun => {
                self.state.set_mode(pb::EngineMode::DryRun);
                ack_ok("dry-run mode enabled")
            }
            pb::EngineMode::Unspecified => ack_err("mode unspecified"),
        };
        Ok(Response::new(ack))
    }

    async fn emergency_stop(
        &self,
        req: Request<pb::EmergencyStopRequest>,
    ) -> Result<Response<pb::Ack>, Status> {
        let req = req.into_inner();
        self.state.emergency_stop();
        Ok(Response::new(ack_ok(format!(
            "emergency stop engaged: {}",
            req.reason
        ))))
    }

    async fn reload_config(
        &self,
        _req: Request<pb::ReloadConfigRequest>,
    ) -> Result<Response<pb::Ack>, Status> {
        Ok(Response::new(ack_ok("config reload requested")))
    }
}

/// Config service: read and optimistically-locked update.
pub struct ConfigSvc {
    state: Arc<ControlState>,
}

impl ConfigSvc {
    /// Builds the service over shared state.
    #[must_use]
    pub fn new(state: Arc<ControlState>) -> ConfigSvc {
        ConfigSvc { state }
    }
}

#[tonic::async_trait]
impl pb::config_server::Config for ConfigSvc {
    async fn get_config(
        &self,
        _req: Request<pb::GetConfigRequest>,
    ) -> Result<Response<pb::EngineConfig>, Status> {
        Ok(Response::new(self.state.config()))
    }

    async fn update_config(
        &self,
        req: Request<pb::UpdateConfigRequest>,
    ) -> Result<Response<pb::Ack>, Status> {
        let req = req.into_inner();
        let Some(config) = req.config else {
            return Ok(Response::new(ack_err("missing config")));
        };
        let ack = match self.state.update_config(config, req.expected_version) {
            Some(version) => ack_ok(format!("config updated to version {version}")),
            None => ack_err("version conflict; reload the current config and retry"),
        };
        Ok(Response::new(ack))
    }
}

/// History service: recent trades, opportunities, and PnL. Backed by the
/// in-memory session rings today, TimescaleDB when that infra is wired.
pub struct HistorySvc {
    state: Arc<ControlState>,
}

impl HistorySvc {
    /// Builds the service over shared state.
    #[must_use]
    pub fn new(state: Arc<ControlState>) -> HistorySvc {
        HistorySvc { state }
    }
}

fn limit_or_default(limit: u32) -> usize {
    if limit == 0 {
        100
    } else {
        limit as usize
    }
}

#[tonic::async_trait]
impl pb::history_server::History for HistorySvc {
    async fn get_trades(
        &self,
        req: Request<pb::GetTradesRequest>,
    ) -> Result<Response<pb::TradesResponse>, Status> {
        let limit = limit_or_default(req.into_inner().limit);
        Ok(Response::new(pb::TradesResponse {
            trades: self.state.recent_trades(limit),
        }))
    }

    async fn get_opportunities(
        &self,
        req: Request<pb::GetOpportunitiesRequest>,
    ) -> Result<Response<pb::OpportunitiesResponse>, Status> {
        let req = req.into_inner();
        let limit = limit_or_default(req.limit);
        Ok(Response::new(pb::OpportunitiesResponse {
            opportunities: self.state.recent_opportunities(limit, req.min_ev),
        }))
    }

    async fn get_pnl_series(
        &self,
        req: Request<pb::GetPnlSeriesRequest>,
    ) -> Result<Response<pb::PnlSeriesResponse>, Status> {
        let since = req.into_inner().since.map_or(0, |t| t.unix_nanos);
        Ok(Response::new(pb::PnlSeriesResponse {
            points: self.state.pnl_series(since),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pb::engine_control_server::EngineControl;

    fn svc() -> (EngineControlSvc, Arc<ControlState>) {
        let state = Arc::new(ControlState::new());
        (EngineControlSvc::new(state.clone()), state)
    }

    #[tokio::test]
    async fn set_mode_live_requires_confirmation() {
        let (svc, state) = svc();
        // Wrong phrase is rejected and mode stays dry-run.
        let resp = svc
            .set_mode(Request::new(pb::SetModeRequest {
                mode: pb::EngineMode::Live as i32,
                confirmation_phrase: "nope".into(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(!resp.ok);
        assert_eq!(state.mode(), pb::EngineMode::DryRun);

        // Correct phrase enables live.
        let resp = svc
            .set_mode(Request::new(pb::SetModeRequest {
                mode: pb::EngineMode::Live as i32,
                confirmation_phrase: LIVE_CONFIRMATION.into(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(resp.ok);
        assert_eq!(state.mode(), pb::EngineMode::Live);
    }

    #[tokio::test]
    async fn emergency_stop_forces_dry_run() {
        let (svc, state) = svc();
        state.set_mode(pb::EngineMode::Live);
        svc.emergency_stop(Request::new(pb::EmergencyStopRequest {
            reason: "test".into(),
            unwind: false,
        }))
        .await
        .unwrap();
        assert!(state.is_emergency());
        assert_eq!(state.mode(), pb::EngineMode::DryRun);
    }

    #[tokio::test]
    async fn config_update_optimistic_locking() {
        use pb::config_server::Config as _;
        let state = Arc::new(ControlState::new());
        let svc = ConfigSvc::new(state.clone());
        let mut config = state.config();
        config.min_edge_bps = 12.0;

        // Stale expected version fails.
        let bad = svc
            .update_config(Request::new(pb::UpdateConfigRequest {
                config: Some(config.clone()),
                expected_version: 999,
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(!bad.ok);

        // Correct version succeeds.
        let good = svc
            .update_config(Request::new(pb::UpdateConfigRequest {
                config: Some(config),
                expected_version: 1,
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(good.ok);
        assert_eq!(state.config().version, 2);
        assert_eq!(state.config().min_edge_bps, 12.0);
    }
}
