//! Control plane: gRPC (unary control and history) and a multiplexed WebSocket
//! (streaming) API for the frontend.
//!
//! The `.proto` files under `proto/` are the single source of truth; the Rust
//! server stubs in [`pb`] are generated from them, with serde derived so the
//! same message types serialize to JSON over the WebSocket. The gRPC services
//! ([`services`]) are backed by shared [`state::ControlState`]; streaming flows
//! through the [`hub::Hub`] and [`ws::serve_ws`].
//!
//! The engine holds the [`state::ControlState`] and the [`hub::Hub`], mutating
//! state as operators issue commands and publishing events as the pipeline runs.
#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;

/// Generated protobuf types and service stubs.
pub mod pb {
    tonic::include_proto!("arbengine.v1");
}

pub mod hub;
pub mod services;
pub mod state;
pub mod ws;

pub use hub::{Hub, StreamEvent};
pub use services::{ConfigSvc, EngineControlSvc, HistorySvc, LIVE_CONFIRMATION};
pub use state::ControlState;
pub use ws::serve_ws;

/// Serves the gRPC control and history API on `addr` until it errors.
pub async fn serve_grpc(
    addr: SocketAddr,
    state: Arc<ControlState>,
) -> Result<(), tonic::transport::Error> {
    use pb::config_server::ConfigServer;
    use pb::engine_control_server::EngineControlServer;
    use pb::history_server::HistoryServer;

    tracing::info!(%addr, "grpc control server listening");
    tonic::transport::Server::builder()
        .add_service(EngineControlServer::new(EngineControlSvc::new(Arc::clone(
            &state,
        ))))
        .add_service(ConfigServer::new(ConfigSvc::new(Arc::clone(&state))))
        .add_service(HistoryServer::new(HistorySvc::new(state)))
        .serve(addr)
        .await
}

#[cfg(test)]
mod tests {
    use super::pb;
    use prost::Message;

    #[test]
    fn proto_message_prost_roundtrip() {
        let opp = pb::Opportunity {
            id: "opp-1".into(),
            path: vec!["USDT".into(), "BTC".into(), "ETH".into(), "USDT".into()],
            legs: Vec::new(),
            gross_product: 1.0021,
            edge_bps: 21.0,
            ev_quote: 3.4,
            size_quote: 500.0,
            confidence: 0.8,
            mode: "parallel_ioc".into(),
            status: "live".into(),
            ts: None,
        };
        let bytes = opp.encode_to_vec();
        let decoded = pb::Opportunity::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded, opp);
    }

    #[test]
    fn proto_message_serde_roundtrip() {
        let alert = pb::Alert {
            id: "a".into(),
            level: "error".into(),
            source: "exec".into(),
            message: "reject".into(),
            ts: None,
        };
        let json = serde_json::to_string(&alert).unwrap();
        let back: pb::Alert = serde_json::from_str(&json).unwrap();
        assert_eq!(back, alert);
    }
}
