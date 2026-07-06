//! The WebSocket streaming server.
//!
//! Accepts browser connections and forwards [`StreamEvent`]s from the [`Hub`] as
//! JSON text frames, one multiplexed socket per client. A lagging client drops
//! intermediate messages rather than stalling the publisher. Client-to-server
//! messages are ignored today (the control actions go over gRPC); pings are
//! answered so idle sockets stay open.

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::error::RecvError;
use tokio_tungstenite::tungstenite::Message;

use crate::hub::Hub;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Binds `addr` and serves the streaming hub to WebSocket clients until the
/// listener errors.
pub async fn serve_ws(addr: SocketAddr, hub: Arc<Hub>) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "websocket streaming server listening");
    loop {
        let (stream, peer) = listener.accept().await?;
        let hub = Arc::clone(&hub);
        tokio::spawn(async move {
            if let Err(e) = handle(stream, hub).await {
                tracing::debug!(%peer, error = %e, "websocket client ended");
            }
        });
    }
}

async fn handle(stream: TcpStream, hub: Arc<Hub>) -> Result<(), BoxError> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut write, mut read) = ws.split();
    let mut rx = hub.subscribe();

    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Ok(ev) => {
                    let json = serde_json::to_string(&ev)?;
                    write.send(Message::Text(json)).await?;
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },
            incoming = read.next() => match incoming {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(Message::Ping(payload))) => {
                    write.send(Message::Pong(payload)).await?;
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.into()),
            },
        }
    }
    Ok(())
}
