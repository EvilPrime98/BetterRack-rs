//! Close guard (`electron/events/close-guard.event.ts` + `services/close-guard.service.ts`).
//! Closing the window cancels running downloads, so when any are `queued` or `running` the user is
//! asked first.

use gpui::AsyncApp;

use crate::api::ApiClient;
use crate::runtime;
use crate::ui::confirm::{self, ConfirmOptions};

/// The dialog text for `active` (> 0) running downloads.
pub fn describe_active_downloads(active: usize) -> String {
    let (noun, it) = if active == 1 { ("download is", "it") } else { ("downloads are", "them") };
    format!("({active}) {noun} still in progress. Closing BetterRack will cancel {it}.")
}

/// Whether the window may close now: at once when nothing is downloading (or the server cannot be
/// asked, since then there is nothing to protect), otherwise after the user answers the dialog.
pub async fn confirm_close(client: ApiClient, cx: &mut AsyncApp) -> bool {
    let active = runtime::run(async move { client.active_download_count().await }).await.unwrap_or_else(|e| {
        tracing::warn!("close guard could not read the download jobs: {e}");
        0
    });
    if active == 0 {
        return true;
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    cx.update(|cx| {
        let options = ConfirmOptions::new("Downloads in progress", describe_active_downloads(active))
            .labels("Close anyway", "Keep open");
        confirm::ask(cx, options, move |answer, _, _| {
            let _ = tx.send(answer == Some(true));
        });
    });
    rx.await.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wording_matches_the_react_dialog() {
        assert_eq!(
            describe_active_downloads(1),
            "(1) download is still in progress. Closing BetterRack will cancel it."
        );
        assert_eq!(
            describe_active_downloads(3),
            "(3) downloads are still in progress. Closing BetterRack will cancel them."
        );
    }
}
