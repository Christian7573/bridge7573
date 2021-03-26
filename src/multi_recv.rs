use async_std::channel::{Sender, Receiver, unbounded};
use async_std::task::spawn;
use futures::FutureExt;
use futures::StreamExt;
use futures::Stream;
use std::sync::Arc;

pub struct MultiRecv<T> {
    to_me: Receiver<Arc<T>>,
    new_receivers: Sender<Sender<Arc<T>>>
}
impl<T> MultiRecv<T> where T: Sync + std::marker::Send + 'static {
    pub fn new() -> (Sender<T>, MultiRecv<T>) {
        let (sender, mut origional_receiver) = unbounded::<T>();
        let (new_receivers, mut receive_new_receivers) = unbounded::<Sender<Arc<T>>>();
        spawn(async move {
            let mut receivers = Vec::new();
            loop {
                futures::select_biased! {
                    new_receiver = receive_new_receivers.next().fuse() => {
                        if let Some(new_receiver) = new_receiver {
                            receivers.push(new_receiver);
                        }
                    },
                    new_msg = origional_receiver.next().fuse() => {
                        if let Some(new_msg) = new_msg {
                            let new_msg = Arc::new(new_msg);
                            let mut i = 0;
                            while i < receivers.len() {
                                if let Err(_) = receivers[i].send(new_msg.clone()).await {
                                    receivers.remove(i);
                                } else {
                                    i += 1;
                                }
                            }
                        } else {
                            return;
                        }
                    }
                }
            }
        });
        let (to_receiver, receiver) = unbounded::<Arc<T>>();
        new_receivers.try_send(to_receiver).expect("How did we get here");
        (sender, MultiRecv { to_me: receiver, new_receivers })
    }
}
impl<T> Clone for MultiRecv<T> where T: Sync + std::marker::Send + 'static {
    fn clone(&self) -> Self {
        let (send_to_me, to_me) = unbounded::<Arc<T>>();
        self.new_receivers.try_send(send_to_me).expect("Failed to clone MultiRecv");
        MultiRecv { to_me, new_receivers: self.new_receivers.clone() }
    }
}
impl<T> Stream for MultiRecv<T> where T: Clone + Sync + std::marker::Send + 'static {
    type Item = Arc<T>;
    fn poll_next(mut self: std::pin::Pin<&mut Self>, ctx: &mut std::task::Context) -> std::task::Poll<Option<Arc<T>>> {
        self.to_me.poll_next_unpin(ctx)
    }
}
