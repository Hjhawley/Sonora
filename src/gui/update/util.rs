//! gui/update/util.rs
use iced::futures::channel::oneshot;

/// Run a blocking function on a background thread and await the result.
///
/// This is intentionally tiny: it avoids repeating the oneshot + thread boilerplate
/// for every “do work off-thread, then send Message::Finished(Result<...>)” case.
pub(crate) async fn spawn_blocking<T>(f: impl FnOnce() -> T + Send + 'static) -> T
where
    T: Send + 'static,
{
    let (tx, rx) = oneshot::channel::<T>();

    std::thread::spawn(move || {
        let _ = tx.send(f());
    });

    rx.await
        .expect("background worker dropped without returning")
}
