// Channel shim that re-exports crossbeam-channel types for normal builds and
// provides a shuttle-aware bounded channel for `cfg(feature = "shuttle-testing")` builds.
//
// The shuttle implementation uses `shuttle::sync::Mutex` so that channel
// send/receive operations are visible to shuttle's scheduler.

// ── non-shuttle (normal build) ───────────────────────────────────────────────

#[cfg(not(feature = "shuttle-testing"))]
pub(crate) use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};

// ── shuttle build ─────────────────────────────────────────────────────────────

#[cfg(feature = "shuttle-testing")]
mod shuttle_impl {
    use shuttle::sync::Mutex;
    use std::{collections::VecDeque, sync::Arc};

    struct ChannelInner<T> {
        buf: VecDeque<T>,
        capacity: usize,
        sender_count: usize,
        receiver_dropped: bool,
    }

    type Inner<T> = Arc<Mutex<ChannelInner<T>>>;

    pub(crate) struct Sender<T>(Inner<T>);
    pub(crate) struct Receiver<T>(Inner<T>);

    pub(crate) enum TrySendError<T> {
        Full(T),
        Disconnected(T),
    }

    impl<T> std::fmt::Debug for TrySendError<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TrySendError::Full(_) => write!(f, "TrySendError::Full(..)"),
                TrySendError::Disconnected(_) => write!(f, "TrySendError::Disconnected(..)"),
            }
        }
    }

    pub(crate) enum TryRecvError {
        Empty,
        Disconnected,
    }

    #[allow(dead_code)]
    pub(crate) struct SendError<T>(pub T);

    impl<T> std::fmt::Debug for SendError<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "SendError(..)")
        }
    }

    pub(crate) fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(Mutex::new(ChannelInner {
            buf: VecDeque::new(),
            capacity,
            sender_count: 1,
            receiver_dropped: false,
        }));
        (Sender(Arc::clone(&inner)), Receiver(inner))
    }

    impl<T> Sender<T> {
        pub(crate) fn len(&self) -> usize {
            self.0.lock().unwrap().buf.len()
        }

        pub(crate) fn try_send(&self, val: T) -> Result<(), TrySendError<T>> {
            let mut inner = self.0.lock().unwrap();
            if inner.receiver_dropped {
                return Err(TrySendError::Disconnected(val));
            }
            if inner.buf.len() >= inner.capacity {
                return Err(TrySendError::Full(val));
            }
            inner.buf.push_back(val);
            Ok(())
        }

        // Blocking send: ignores capacity limit so tests don't deadlock under
        // shuttle's scheduler (channel capacity is 384, so in practice tests
        // never fill it before draining).
        #[allow(dead_code)]
        pub(crate) fn send(&self, val: T) -> Result<(), SendError<T>> {
            let mut inner = self.0.lock().unwrap();
            if inner.receiver_dropped {
                return Err(SendError(val));
            }
            inner.buf.push_back(val);
            Ok(())
        }
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            self.0.lock().unwrap().sender_count += 1;
            Sender(Arc::clone(&self.0))
        }
    }

    impl<T> Drop for Sender<T> {
        fn drop(&mut self) {
            self.0.lock().unwrap().sender_count -= 1;
        }
    }

    // Safety: Sender<T> contains Arc<Mutex<...>> which is Send + Sync when T: Send.
    unsafe impl<T: Send> Send for Sender<T> {}
    unsafe impl<T: Send> Sync for Sender<T> {}

    impl<T> Receiver<T> {
        pub(crate) fn len(&self) -> usize {
            self.0.lock().unwrap().buf.len()
        }

        pub(crate) fn try_recv(&self) -> Result<T, TryRecvError> {
            let mut inner = self.0.lock().unwrap();
            match inner.buf.pop_front() {
                Some(val) => Ok(val),
                None => {
                    if inner.sender_count == 0 {
                        Err(TryRecvError::Disconnected)
                    } else {
                        Err(TryRecvError::Empty)
                    }
                }
            }
        }
    }

    impl<T> Drop for Receiver<T> {
        fn drop(&mut self) {
            self.0.lock().unwrap().receiver_dropped = true;
        }
    }

    // Safety: Receiver<T> contains Arc<Mutex<...>> which is Send when T: Send.
    unsafe impl<T: Send> Send for Receiver<T> {}
}

#[cfg(feature = "shuttle-testing")]
pub(crate) use shuttle_impl::{bounded, Receiver, Sender, TrySendError};
