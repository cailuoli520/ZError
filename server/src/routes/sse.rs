//! SSE：管理后台实时日志流。

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::logger::SSEEvent;
use crate::state::AppState;

pub async fn logs_stream(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.logger.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|item| match item {
        Ok(ev) => to_event(&ev),
        Err(BroadcastStreamRecvError::Lagged(n)) => {
            tracing::warn!("SSE 订阅者滞后，丢弃 {} 条事件", n);
            None
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("keepalive"))
}

fn to_event(ev: &SSEEvent) -> Option<Result<Event, Infallible>> {
    let data = serde_json::to_string(ev).ok()?;
    Some(Ok(Event::default().event(ev.event_name()).data(data)))
}
