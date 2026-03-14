use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll},
};

use tokio::sync::{futures::Notified, Notify};

/// Helper structure that wraps atomic bool to signal shim server when to shutdown the TTRPC server.
///
/// Shim implementations are responsible for calling [`Self::signal`].
pub struct ExitSignal {
    notifier: Notify,
    exited: AtomicBool,
}

impl Default for ExitSignal {
    fn default() -> Self {
        ExitSignal {
            notifier: Notify::new(),
            exited: AtomicBool::new(false),
        }
    }
}

impl ExitSignal {
    /// Set exit signal to shutdown shim server.
    pub fn signal(&self) {
        self.exited.store(true, Ordering::SeqCst);
        self.notifier.notify_waiters();
    }

    /// Wait for the exit signal to be set.
    pub async fn wait(&self) {
        loop {
            let notified = self.notifier.notified();
            if self.exited.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }

    pub fn exited(&self) -> Exited {
        let notified = self.notifier.notified();
        Exited {
            notified,
            sig: self,
        }
    }
}

pin_project_lite::pin_project! {
    pub struct Exited<'a> {
        #[pin]
        notified: Notified<'a>,
        sig: &'a ExitSignal,
    }
}

impl Future for Exited<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if this.sig.exited.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        this.notified.poll(cx)
    }
}
