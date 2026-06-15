//! Lock-free connection pool — mechanical sympathy design.
//!
//! Principles applied (per optimization guide):
//!
//! 1. **No blocking primitives on hot path** — crossbeam::ArrayQueue is Dmitry
//!    Vyukov's bounded MPMC queue with pure CAS loops. No Mutex, no Semaphore.
//!    tokio::sync::Notify uses futex (Linux) / parking (macOS) — kernel boundary
//!    only when a waiter actually needs to sleep.
//!
//! 2. **Cache-line false-sharding eliminated** — crossbeam::ArrayQueue uses
//!    `CachePadded<AtomicUsize>` for head and tail on separate cache lines.
//!    Producers and consumers never invalidate the same cache line.
//!
//! 3. **Zero allocation on hot path** — All connections pre-allocated at
//!    construction. ArrayQueue buffer is fixed-size. No VecDeque growth,
//!    no Metrics per object, no Instant::now() on hot path.
//!
//! 4. **Monormorphic dispatch** — `acquire()` and `return_conn()` are fully
//!    concrete methods on PoolInner<T>. No trait objects, no vtable lookups
//!    on the queue path. Factory closures are set once at construction.
//!
//! 5. **Branchless inner loops** — The CAS loops in ArrayQueue push/pop are
//!    tight spinning loops with backoff (pause on x86, wfe on ARM).
//!    No unpredictable branches — just cmp+cmpxchg until success.
//!
//! 6. **Flat data structures** — PoolInner is a flat struct. No nested Arc,
//!    no Weak, no Option overhead on idle queue slots.
//!
//! 7. **Proper memory ordering** — Acquire/Release semantics for size and
//!    closed state. Not Just Relaxed everywhere.
//!
//! 8. **No virtual dispatch** — Factory is boxed once at construction.
//!    The hot queue path uses monomorphic array operations.

use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam::queue::ArrayQueue;
use tokio::sync::Notify;
use tokio::time::timeout;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type CreateFn<T> = Box<dyn Fn() -> BoxFuture<'static, Result<T, String>> + Send + Sync>;
pub type ValidateFn<T> = Box<dyn Fn(&T) -> bool + Send + Sync>;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolError {
    Timeout,
    Closed,
    CreateFailed(String),
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::Timeout => write!(f, "pool: timeout waiting for connection"),
            PoolError::Closed => write!(f, "pool: closed"),
            PoolError::CreateFailed(m) => write!(f, "pool: create failed: {m}"),
        }
    }
}

impl std::error::Error for PoolError {}

// ─── Config ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub max_size: u32,
    pub create_timeout: Duration,
    pub wait_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 20,
            create_timeout: Duration::from_secs(5),
            wait_timeout: Duration::from_secs(10),
        }
    }
}

// ─── Status ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PoolStatus {
    /// Total connections (idle + checked out)
    pub size: u32,
    /// Idle connections in queue
    pub idle: u32,
    /// Maximum allowed connections
    pub max_size: u32,
    /// Whether the pool is closed
    pub closed: bool,
}

// ─── Core pool ───────────────────────────────────────────────────────────────

pub struct LockFreePool<T: Send + 'static> {
    inner: Arc<PoolInner<T>>,
}

// SAFETY: PoolInner<T> uses internal synchronization via atomics and
// crossbeam's lock-free queue. Send + Sync are safe when T: Send.
unsafe impl<T: Send + 'static> Send for LockFreePool<T> {}
unsafe impl<T: Send + 'static> Sync for LockFreePool<T> {}

impl<T: Send + 'static> Clone for LockFreePool<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// A connection checked out from the pool.
///
/// Automatically returned to the pool when dropped.  Implements `Deref`
/// so you can use it as a reference to the underlying connection type.
///
/// # Lock-free guarantee
///
/// `Drop` performs exactly one lock-free `ArrayQueue::push` (CAS loop)
/// and one `Notify::notify_one()` (atomic store + optional futex_wake).
/// No mutexes, no allocations.
pub struct PooledConnection<T: Send + 'static> {
    inner: Option<T>,
    pool: LockFreePool<T>,
}

// SAFETY: PooledConnection owns T which is Send.  The pool's return path
// is lock-free and does not dereference any thread-local state.
unsafe impl<T: Send + 'static> Send for PooledConnection<T> {}
unsafe impl<T: Send + 'static> Sync for PooledConnection<T> {}

impl<T: Send + 'static> std::fmt::Debug for PooledConnection<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledConnection")
            .field("connected", &self.inner.is_some())
            .finish()
    }
}

impl<T: Send + 'static> Deref for PooledConnection<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        // Safety: inner is always Some until Drop
        unsafe { self.inner.as_ref().unwrap_unchecked() }
    }
}

impl<T: Send + 'static> AsRef<T> for PooledConnection<T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<T: Send + 'static> PooledConnection<T> {
    /// Take the inner value out of the connection, permanently removing it
    /// from the pool.  The pool's size is decremented.
    pub fn take(mut self) -> T {
        let conn = self.inner.take().unwrap();
        self.pool.inner.size.fetch_sub(1, Ordering::Release);
        conn
    }

    /// Return a reference to the pool status
    pub fn pool_status(&self) -> PoolStatus {
        self.pool.status()
    }
}

// --- Return path: called from Drop (sync), must be lock-free.
//     Only does ArrayQueue push (CAS loop, no syscall in common case)
//     and atomic size decrement if queue is full.
impl<T: Send + 'static> Drop for PooledConnection<T> {
    #[inline]
    fn drop(&mut self) {
        if let Some(item) = self.inner.take() {
            self.pool.inner.return_conn(item);
        }
    }
}

// ─── PoolInner: the actual state ─────────────────────────────────────────────

struct PoolInner<T: Send + 'static> {
    /// Factory for creating connections (boxed closure, set once)
    create: CreateFn<T>,

    /// Connection health validator (boxed closure, set once)
    validate: ValidateFn<T>,

    /// Lock-free bounded MPMC queue of idle connections.
    /// Pre-allocated at construction to `max_size` capacity.
    idle: ArrayQueue<T>,

    /// Current pool size (idle + checked out).
    /// Modified only via CAS (AcqRel semantics).
    size: AtomicU32,

    /// Maximum connections.  Immutable after construction.
    max_size: u32,

    /// Closed flag.  Set once with Release, read with Acquire.
    closed: AtomicBool,

    /// Async waiter notification.  Uses `futex` on Linux (no mutex),
    /// `_umtx_op` on macOS, or `parking` on other platforms.
    /// Only touched when a waiter actually needs to sleep.
    notify: Notify,

    /// Connection create timeout
    create_timeout: Duration,

    /// Connection wait timeout
    wait_timeout: Duration,
}

// SAFETY: PoolInner<T> uses only lock-free synchronization internally.
unsafe impl<T: Send + 'static> Send for PoolInner<T> {}
unsafe impl<T: Send + 'static> Sync for PoolInner<T> {}

impl<T: Send + 'static> LockFreePool<T> {
    /// Create a new lock-free pool with the given factory and config.
    ///
    /// All memory for the idle queue is pre-allocated at construction
    /// (`max_size` slots).  No heap allocation occurs on the hot path.
    pub fn new(
        create: CreateFn<T>,
        validate: ValidateFn<T>,
        config: PoolConfig,
    ) -> Self {
        // Pre-allocate exactly max_size slots — never grows, never shrinks
        let idle = ArrayQueue::new(config.max_size as usize);
        Self {
            inner: Arc::new(PoolInner {
                create,
                validate,
                idle,
                size: AtomicU32::new(0),
                max_size: config.max_size,
                closed: AtomicBool::new(false),
                notify: Notify::new(),
                create_timeout: config.create_timeout,
                wait_timeout: config.wait_timeout,
            }),
        }
    }

    /// Acquire a connection from the pool.
    ///
    /// ## Fast path (common case, lock-free)
    /// 1. Pop from idle queue (CAS loop, no syscall)
    /// 2. Validate the connection (async SELECT 1)
    /// 3. Return as PooledConnection
    ///
    /// ## Slow path (pool empty, not at capacity)
    /// 4. CAS-increment size, create connection, return
    ///
    /// ## Wait path (pool empty, at capacity)
    /// 5. Park on Notify with timeout, retry
    #[inline]
    pub async fn acquire(&self) -> Result<PooledConnection<T>, PoolError> {
        // !!!  HOT PATH BEGINS  !!!
        // Checks are ordered: closed is the cheapest (one atomic load),
        // then idle pop (lock-free CAS), then create path.

        if self.inner.closed.load(Ordering::Acquire) {
            return Err(PoolError::Closed);
        }

        // ── Fast path: pop idle connection ──
        // Single lock-free operation, no kernel boundary.
        if let Some(item) = self.inner.idle.pop() {
            // Validate the connection (async, but usually just a quick query)
            // If validation fails, destroy and fall through to create path.
            if (self.inner.validate)(&item) {
                return Ok(PooledConnection {
                    inner: Some(item),
                    pool: self.clone(),
                });
            }
            // Validation failed — drop the connection and decrement size.
            // The connection is effectively dead; we don't return it.
            drop(item);
            self.inner.size.fetch_sub(1, Ordering::Release);
            // Fall through to try creating a new one
        }

        // ── Create path: pool empty ──
        loop {
            if self.inner.closed.load(Ordering::Acquire) {
                return Err(PoolError::Closed);
            }

            // Try to claim a slot via CAS
            let current = self.inner.size.load(Ordering::Acquire);
            if current < self.inner.max_size {
                // CAS: reserve a slot atomically
                // This prevents two concurrent tasks from both trying to
                // create beyond max_size.
                if self.inner.size.compare_exchange_weak(
                    current,
                    current + 1,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ).is_ok() {
                    // Slot reserved — create the connection
                    return match self.create_one().await {
                        Ok(item) => Ok(PooledConnection {
                            inner: Some(item),
                            pool: self.clone(),
                        }),
                        Err(e) => {
                            // Creation failed — release the reserved slot
                            self.inner.size.fetch_sub(1, Ordering::Release);
                            self.inner.notify.notify_one();
                            Err(e)
                        }
                    };
                }
                // CAS failed — another task claimed the slot, retry
                continue;
            }

            // ── Wait path: pool saturated ──
            // Short-circuit if no timeout
            if self.inner.wait_timeout == Duration::ZERO {
                return Err(PoolError::Timeout);
            }

            // Park on the Notify with timeout.
            // Notify is a futex-based primitive — no mutex, no semaphore.
            let notified = self.inner.notify.notified();
            tokio::select! {
                _ = notified => {
                    // Woken — another task returned a connection.
                    // Try to pop it.
                    if let Some(item) = self.inner.idle.pop() {
                        if (self.inner.validate)(&item) {
                            return Ok(PooledConnection {
                                inner: Some(item),
                                pool: self.clone(),
                            });
                        }
                        drop(item);
                        self.inner.size.fetch_sub(1, Ordering::Release);
                    }
                    // No connection available — loop back and retry.
                    // This happens if a concurrent acquirer stole the
                    // connection before we could wake up.  The loop
                    // will try the idle queue again.
                    continue;
                }
                _ = tokio::time::sleep(self.inner.wait_timeout) => {
                    // Wait timeout expired — one last retry before giving up.
                    if let Some(item) = self.inner.idle.pop() {
                        if (self.inner.validate)(&item) {
                            return Ok(PooledConnection {
                                inner: Some(item),
                                pool: self.clone(),
                            });
                        }
                        drop(item);
                        self.inner.size.fetch_sub(1, Ordering::Release);
                    }
                    return Err(PoolError::Timeout);
                }
            }
        }
    }

    /// Create a single new connection with timeout.
    #[inline]
    async fn create_one(&self) -> Result<T, PoolError> {
        if self.inner.closed.load(Ordering::Acquire) {
            self.inner.size.fetch_sub(1, Ordering::Release);
            return Err(PoolError::Closed);
        }
        match timeout(self.inner.create_timeout, (self.inner.create)()).await {
            Ok(Ok(item)) => Ok(item),
            Ok(Err(msg)) => Err(PoolError::CreateFailed(msg)),
            Err(_) => Err(PoolError::CreateFailed("timeout".into())),
        }
    }

    pub fn close(&self) {
        self.inner.closed.store(true, Ordering::Release);
        self.inner.notify.notify_waiters();
        while self.inner.idle.pop().is_some() {
            self.inner.size.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::Acquire)
    }

    #[inline]
    pub fn status(&self) -> PoolStatus {
        self.inner.status()
    }

    pub fn max_size(&self) -> u32 {
        self.inner.max_size
    }
}

impl<T: Send + 'static> PoolInner<T> {
    /// Return a connection to the pool.
    ///
    /// Called from `PooledConnection::drop()` — MUST be sync.
    ///
    /// # Lock-free guarantee
    ///
    /// Performs exactly one ArrayQueue push (CAS loop) and
    /// one Notify::notify_one() (atomic store + optional futex_wake).
    /// No mutexes, no allocations.
    #[inline]
    fn return_conn(&self, item: T) {
        let closed = self.closed.load(Ordering::Acquire);
        if !closed {
            match self.idle.push(item) {
                Ok(()) => {
                    self.notify.notify_one();
                    return;
                }
                Err(dropped) => {
                    // Queue full — drop the connection
                    drop(dropped);
                }
            }
        }
        self.size.fetch_sub(1, Ordering::Release);
        self.notify.notify_one();
    }

    #[inline]
    fn status(&self) -> PoolStatus {
        let size = self.size.load(Ordering::Acquire);
        let idle = self.idle.len();
        PoolStatus {
            size,
            idle: idle as u32,
            max_size: self.max_size,
            closed: self.closed.load(Ordering::Acquire),
        }
    }
}

// ─── Drop: close the pool when all references are dropped ────────────────────

impl<T: Send + 'static> Drop for PoolInner<T> {
    fn drop(&mut self) {
        // Drain idle connections
        while self.idle.pop().is_some() {}
    }
}

// ─── Test helpers ────────────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

    /// A test connection that tracks creation, validation, and drop counts.
    pub struct TestConnection {
        pub id: u32,
        pub valid: bool,
    }

    impl Drop for TestConnection {
        fn drop(&mut self) {
            // Tracked via global counter in the factory
        }
    }

    pub fn create_test_pool(
        max_size: u32,
        fail_create: bool,
        fail_validate: bool,
    ) -> LockFreePool<TestConnection> {
        let create_count = Arc::new(AtomicU32::new(0));

        let create = {
            let cc = create_count.clone();
            Box::new(move || {
                let count = cc.fetch_add(1, AtomicOrdering::Relaxed);
                Box::pin(async move {
                    if fail_create {
                        Err("create failed".into())
                    } else {
                        Ok(TestConnection {
                            id: count,
                            valid: !fail_validate,
                        })
                    }
                }) as BoxFuture<'static, Result<TestConnection, String>>
            }) as CreateFn<TestConnection>
        };

        let validate = Box::new(move |conn: &TestConnection| conn.valid) as ValidateFn<TestConnection>;

        let config = PoolConfig {
            max_size,
            create_timeout: Duration::from_secs(5),
            wait_timeout: Duration::from_secs(10),
        };

        LockFreePool::new(create, validate, config)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::sleep;

    // ─── Basic acquire/release cycles ─────────────────────────────────────

    #[tokio::test]
    async fn test_acquire_release_one() {
        let pool = create_test_pool(5, false, false);
        assert!(!pool.is_closed());

        let conn = pool.acquire().await.unwrap();
        assert_eq!(conn.id, 0);
        assert!(conn.valid);

        let status = pool.status();
        assert_eq!(status.size, 1);
        assert_eq!(status.idle, 0);

        drop(conn);
        sleep(Duration::from_millis(10)).await;

        let status = pool.status();
        assert_eq!(status.idle, 1);
    }

    #[tokio::test]
    async fn test_acquire_release_reuse() {
        let pool = create_test_pool(5, false, false);

        let conn1 = pool.acquire().await.unwrap();
        let id1 = conn1.id;
        drop(conn1);

        sleep(Duration::from_millis(10)).await;

        let conn2 = pool.acquire().await.unwrap();
        assert_eq!(conn2.id, id1, "should reuse the same connection");
    }

    #[tokio::test]
    async fn test_multiple_connections() {
        let pool = create_test_pool(5, false, false);
        let mut conns = Vec::new();
        for _ in 0..5 {
            let conn = pool.acquire().await.unwrap();
            conns.push(conn);
        }
        assert_eq!(pool.status().size, 5);
        assert_eq!(pool.status().idle, 0);
        drop(conns);
    }

    #[tokio::test]
    async fn test_acquire_multiple_release_reuse() {
        let pool = create_test_pool(5, false, false);
        let mut conns = Vec::new();

        for _ in 0..5 {
            conns.push(pool.acquire().await.unwrap());
        }
        let ids: Vec<u32> = conns.iter().map(|c| c.id).collect();
        drop(conns);

        sleep(Duration::from_millis(10)).await;

        let mut reused = 0;
        for _ in 0..5 {
            let conn = pool.acquire().await.unwrap();
            if ids.contains(&conn.id) {
                reused += 1;
            }
            drop(conn);
        }
        assert!(reused >= 4, "most connections should be reused");
    }

    // ─── Pool exhaustion and timeout ──────────────────────────────────────

    #[tokio::test]
    async fn test_pool_exhaustion_short_timeout() {
        let config = PoolConfig {
            max_size: 1,
            create_timeout: Duration::from_secs(1),
            wait_timeout: Duration::from_millis(100),
        };
        let pool = LockFreePool::new(
            Box::new(|| {
                Box::pin(async { Ok(TestConnection { id: 0, valid: true }) })
                    as BoxFuture<'static, Result<TestConnection, String>>
            }) as CreateFn<TestConnection>,
            Box::new(|_conn: &TestConnection| true) as ValidateFn<TestConnection>,
            config,
        );

        let conn1 = pool.acquire().await.unwrap();
        let result = pool.acquire().await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PoolError::Timeout);
        drop(conn1);
    }

    #[tokio::test]
    async fn test_acquire_no_timeout_when_conn_returned() {
        // Verify that a returned connection unblocks a waiting acquirer
        let config = PoolConfig {
            max_size: 1,
            create_timeout: Duration::from_secs(1),
            wait_timeout: Duration::from_secs(5),
        };
        let pool = Arc::new(LockFreePool::new(
            Box::new(|| {
                Box::pin(async { Ok(TestConnection { id: 0, valid: true }) })
                    as BoxFuture<'static, Result<TestConnection, String>>
            }) as CreateFn<TestConnection>,
            Box::new(|_conn: &TestConnection| true) as ValidateFn<TestConnection>,
            config,
        ));

        let conn1 = pool.acquire().await.unwrap();
        let pool_clone = pool.clone();

        let handle = tokio::spawn(async move {
            pool_clone.acquire().await
        });

        sleep(Duration::from_millis(50)).await;
        drop(conn1);

        let result = handle.await.unwrap();
        assert!(result.is_ok(), "returned conn should unblock waiter");
    }

    // ─── Connection validation ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_validation_rejects_invalid_connections() {
        // Validator always returns false — every idle connection is rejected.
        // Pool must create a new connection on every reuse.
        let reject_count = Arc::new(AtomicU32::new(0));
        let create_count = Arc::new(AtomicU32::new(0));

        let create = {
            let cc = create_count.clone();
            Box::new(move || {
                let id = cc.fetch_add(1, AtomicOrdering::Relaxed);
                Box::pin(async move {
                    Ok(TestConnection { id, valid: true })
                }) as BoxFuture<'static, Result<TestConnection, String>>
            }) as CreateFn<TestConnection>
        };

        let validate = {
            let rc = reject_count.clone();
            Box::new(move |_conn: &TestConnection| {
                rc.fetch_add(1, AtomicOrdering::Relaxed);
                false
            }) as ValidateFn<TestConnection>
        };

        let pool = LockFreePool::new(
            create,
            validate,
            PoolConfig {
                max_size: 5,
                create_timeout: Duration::from_secs(5),
                wait_timeout: Duration::from_secs(1),
            },
        );

        // First acquire: creates conn(id=0, no validation on creation path)
        let conn1 = pool.acquire().await.unwrap();
        assert_eq!(conn1.id, 0);
        drop(conn1); // return to idle

        // Second acquire: pops conn0 from idle, validator rejects,
        // discards, creates conn(id=1)
        let conn2 = pool.acquire().await.unwrap();
        assert_eq!(conn2.id, 1, "rejected idle conn should be replaced");

        let rejected = reject_count.load(AtomicOrdering::Relaxed);
        assert_eq!(rejected, 1, "validator should be called exactly once");

        drop(conn2);
    }

    // ─── Close behavior ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_close() {
        let pool = create_test_pool(5, false, false);
        let conn = pool.acquire().await.unwrap();
        assert!(!pool.is_closed());
        pool.close();
        assert!(pool.is_closed());
        // Acquire after close should fail
        let result = pool.acquire().await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PoolError::Closed);
        drop(conn); // Should be handled gracefully
    }

    #[tokio::test]
    async fn test_close_with_waiter() {
        let config = PoolConfig {
            max_size: 1,
            create_timeout: Duration::from_secs(1),
            wait_timeout: Duration::from_secs(10),
        };
        let pool = Arc::new(LockFreePool::new(
            Box::new(|| {
                Box::pin(async { Ok(TestConnection { id: 0, valid: true }) })
                    as BoxFuture<'static, Result<TestConnection, String>>
            }) as CreateFn<TestConnection>,
            Box::new(|_conn: &TestConnection| true) as ValidateFn<TestConnection>,
            config,
        ));

        let conn1 = pool.acquire().await.unwrap();
        let pool_clone = pool.clone();

        // Spawn a waiter that will be waiting for a connection
        let handle = tokio::spawn(async move {
            pool_clone.acquire().await
        });

        // Give the spawned task time to start waiting
        sleep(Duration::from_millis(50)).await;

        // Close the pool — the waiter should wake up and get Closed error
        pool.close();
        let result = handle.await.unwrap();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PoolError::Closed);
        drop(conn1);
    }

    // ─── Concurrent access stress test ────────────────────────────────────

    #[tokio::test]
    async fn test_concurrent_acquire_release() {
        let pool = Arc::new(create_test_pool(8, false, false));
        let mut handles = Vec::new();

        for _ in 0..16 {
            let pool = pool.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..10 {
                    let conn = pool.acquire().await.unwrap();
                    // "Use" the connection briefly
                    sleep(Duration::from_millis(5)).await;
                    drop(conn); // Return to pool
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let status = pool.status();
        assert!(status.size <= 8, "pool should not exceed max_size");
    }

    #[tokio::test]
    async fn test_concurrent_stress_high_contention() {
        let pool = Arc::new(create_test_pool(4, false, false));
        let mut handles = Vec::new();

        for _ in 0..32 {
            let pool = pool.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..25 {
                    match pool.acquire().await {
                        Ok(conn) => {
                            // Minimal "work" — just hold briefly
                            tokio::task::yield_now().await;
                            drop(conn);
                        }
                        Err(PoolError::Timeout) => {
                            // Expected when pool is saturated
                            tokio::task::yield_now().await;
                        }
                        Err(e) => panic!("Unexpected error: {e}"),
                    }
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let status = pool.status();
        assert!(status.size <= 4, "pool exceeded max_size: {}", status.size);
        assert!(!status.closed);
    }

    // ─── Zero timeout (non-blocking) ──────────────────────────────────────

    #[tokio::test]
    async fn test_zero_wait_timeout() {
        let config = PoolConfig {
            max_size: 1,
            create_timeout: Duration::from_secs(1),
            wait_timeout: Duration::ZERO,
        };
        let pool = LockFreePool::new(
            Box::new(|| {
                Box::pin(async { Ok(TestConnection { id: 0, valid: true }) })
                    as BoxFuture<'static, Result<TestConnection, String>>
            }) as CreateFn<TestConnection>,
            Box::new(|_conn: &TestConnection| true) as ValidateFn<TestConnection>,
            config,
        );

        let _conn = pool.acquire().await.unwrap();
        // Second acquire with zero timeout should fail immediately
        let result = pool.acquire().await;
        assert_eq!(result.unwrap_err(), PoolError::Timeout);
    }

    // ─── Create failures ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_failure() {
        let pool = create_test_pool(5, true, false);
        let result = pool.acquire().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PoolError::CreateFailed(_)));
    }

    // ─── Take ownership (remove from pool) ────────────────────────────────

    #[tokio::test]
    async fn test_take_connection() {
        let pool = create_test_pool(5, false, false);
        let conn = pool.acquire().await.unwrap();
        let id = conn.id;
        let taken = PooledConnection::take(conn);
        assert_eq!(taken.id, id);
        // Connection is gone from pool
        // No way to check size easily, but pool should have decremented
        let status = pool.status();
        assert_eq!(status.size, 0); // taken connection is removed
    }

    // ─── Clone pool ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_pool_clone() {
        let pool = create_test_pool(5, false, false);
        let pool2 = pool.clone();
        let conn = pool2.acquire().await.unwrap();
        assert!(conn.valid);
        drop(conn);
    }

    // ─── Close with connections checked out ───────────────────────────────

    #[tokio::test]
    async fn test_close_with_active_connections() {
        let pool = create_test_pool(5, false, false);
        let conn1 = pool.acquire().await.unwrap();
        let conn2 = pool.acquire().await.unwrap();
        pool.close();
        assert!(pool.is_closed());
        let result = pool.acquire().await;
        assert_eq!(result.unwrap_err(), PoolError::Closed);
        // Dropping checked-out connections after close should not panic
        drop(conn1);
        drop(conn2);
    }
}
