use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::Notify;

#[derive(Debug, Default)]
pub(crate) struct AudioInFlightLimiter {
    streams: Mutex<BTreeSet<u64>>,
    released: Notify,
}

impl AudioInFlightLimiter {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn try_acquire(&self, stream_id: u64) -> Option<AudioInFlightGuard<'_>> {
        let mut streams = self.streams.lock().ok()?;
        if !streams.insert(stream_id) {
            return None;
        }
        Some(AudioInFlightGuard {
            limiter: self,
            stream_id,
        })
    }

    pub(crate) async fn wait_until_idle(&self, stream_id: u64, timeout: Duration) -> bool {
        tokio::time::timeout(timeout, async {
            loop {
                let released = self.released.notified();
                if !self.is_in_flight(stream_id) {
                    break;
                }
                released.await;
            }
        })
        .await
        .is_ok()
    }

    fn is_in_flight(&self, stream_id: u64) -> bool {
        self.streams
            .lock()
            .is_ok_and(|streams| streams.contains(&stream_id))
    }

    fn release(&self, stream_id: u64) {
        if let Ok(mut streams) = self.streams.lock() {
            streams.remove(&stream_id);
        }
        self.released.notify_waiters();
    }
}

pub(crate) struct AudioInFlightGuard<'a> {
    limiter: &'a AudioInFlightLimiter,
    stream_id: u64,
}

impl Drop for AudioInFlightGuard<'_> {
    fn drop(&mut self) {
        self.limiter.release(self.stream_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn waits_until_stream_guard_is_released() {
        let limiter = AudioInFlightLimiter::new();
        let guard = limiter.try_acquire(9).expect("acquired");
        assert!(limiter.try_acquire(9).is_none());

        let waited = tokio::time::timeout(Duration::from_millis(100), async {
            tokio::task::yield_now().await;
            drop(guard);
            limiter.wait_until_idle(9, Duration::from_secs(1)).await
        })
        .await
        .expect("wait finishes");

        assert!(waited);
        assert!(limiter.try_acquire(9).is_some());
    }

    #[tokio::test]
    async fn wait_times_out_while_stream_is_busy() {
        let limiter = AudioInFlightLimiter::new();
        let _guard = limiter.try_acquire(9).expect("acquired");

        assert!(!limiter.wait_until_idle(9, Duration::from_millis(1)).await);
    }
}
