//! A oneshot channel whose `send` completes only once the receiver has taken
//! the value, so the worker learns whether a `BEGIN`, `COMMIT` or `ROLLBACK` it
//! ran was acknowledged.

use futures_channel::oneshot::{self, Canceled};

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (inner_tx, inner_rx) = oneshot::channel();
    (Sender { inner: inner_tx }, Receiver { inner: inner_rx })
}

pub struct Sender<T> {
    inner: oneshot::Sender<(T, oneshot::Sender<()>)>,
}

impl<T> Sender<T> {
    pub async fn send(self, value: T) -> Result<(), Canceled> {
        let (ack_tx, ack_rx) = oneshot::channel();
        self.inner.send((value, ack_tx)).map_err(|_| Canceled)?;
        ack_rx.await
    }

    pub fn blocking_send(self, value: T) -> Result<(), Canceled> {
        futures_executor::block_on(self.send(value))
    }
}

pub struct Receiver<T> {
    inner: oneshot::Receiver<(T, oneshot::Sender<()>)>,
}

impl<T> Receiver<T> {
    pub async fn recv(self) -> Result<T, Canceled> {
        let (value, ack_tx) = self.inner.await?;
        ack_tx.send(()).map_err(|_| Canceled)?;
        Ok(value)
    }
}
