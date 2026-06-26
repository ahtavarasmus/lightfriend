use super::ChatStatus;

pub(super) fn emit_status(
    status_tx: Option<&tokio::sync::mpsc::Sender<ChatStatus>>,
    status: ChatStatus,
) {
    if let Some(tx) = status_tx {
        let _ = tx.try_send(status);
    }
}

pub(super) fn spawn_reasoning_bridge(
    status_tx: Option<&tokio::sync::mpsc::Sender<ChatStatus>>,
) -> Option<tokio::sync::mpsc::Sender<String>> {
    if let Some(status_tx) = status_tx {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);
        let status_tx = status_tx.clone();
        tokio::spawn(async move {
            while let Some(snippet) = rx.recv().await {
                let _ = status_tx.send(ChatStatus::Reasoning { snippet }).await;
            }
        });
        Some(tx)
    } else {
        None
    }
}
