//! Guarded child-process runner (watchdog, RSS cap, `wait4` rusage, optional CPU pinning, bounded output
//! capture) and the RustSpice subprocess runner built on it.
//!
//! The child is started through `posix_spawn` (no `pre_exec` hook) unless an [`orphan_guard`] is active for the
//! calling thread, in which case `fork` + `PR_SET_PDEATHSIG` is used (correctness runs only: the start-up cost of that
//! path is too high for a timed repetition). CPU pinning is applied to the *calling thread* around `spawn` (the child
//! inherits the mask) and restored afterwards; a failed `sched_setaffinity` is counted ([`pin_failures`]), never
//! hidden.
//!
//! Output is captured as a head and a tail per stream (ngspice prints its timing lines at the end), a pipe that a
//! detached grandchild keeps open costs at most [`READER_GRACE`] and is flagged (`capture_incomplete`).
//!
//! Termination: [`install_termination_cleanup`] makes SIGTERM / SIGINT / SIGHUP kill every registered child process
//! group and remove the registered scratch directories before the process exits with `128 + signal`.
//!
//! Peak memory of a child is NEVER taken from `wait4`'s `ru_maxrss`: a vfork-style `exec` records the old address
//! space's high-water mark (the harness's own) in the child's `ru_maxrss`, so every child reports at least the
//! harness's footprint. The watchdog instead polls `VmHWM` of `/proc/<pid>/status` (the high-water mark of the
//! child's own post-`exec` address space, monotone, so a poll anywhere before the exit sees every earlier peak).
//! Allocations made in the last polling interval before the exit can still be missed, and a process that exits before
//! the first poll has no sample at all: [`ChildRun::max_rss_kib`] is then `None`, never 0.
//!
//! A poll is only trusted when it provably saw the child's own address space ([`sample_child`]): between `spawn`
//! returning and the child's `exec` switching to its new address space, `/proc/<pid>/status` still shows the PARENT's
//! (the vfork-style `exec` wakes the parent first), and one such sample would poison the maximum with the harness's
//! footprint.

use std::cell::Cell;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, Once, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::rawfile::{self, Plot};

/// Output kept per stream (bytes): a head and a tail, the middle of a longer stream is drained and dropped.
pub const CAPTURE_LIMIT: usize = 256 * 1024;
/// Head of a stream kept verbatim (start-up banner, the first message of a failing run).
const CAPTURE_HEAD: usize = 64 * 1024;
/// Tail of a stream kept verbatim. ngspice prints `Total analysis time` / `Total elapsed time` at the very END,
/// after a node table of every node of a `.tran` deck: a head-only capture lost both lines on large decks.
const CAPTURE_TAIL: usize = CAPTURE_LIMIT - CAPTURE_HEAD;
/// How long the pipe readers may keep reading after the leader has exited and its process group was killed. A
/// detached grandchild (`setsid`) can hold the pipes open for good: what was read is returned and flagged
/// ([`ChildRun::capture_incomplete`]).
const READER_GRACE: Duration = Duration::from_millis(750);

/// Exit code of `rustspice run` when it could not WRITE its output (`--raw` on a full or failing disk, ...): an
/// environment failure of the harness, never a refusal of the deck. [`classify_exit`] keeps it apart from a loud
/// error and the suite reports the case `unverified`.
pub const HARNESS_IO_EXIT: i32 = 74;

#[derive(Clone, Debug)]
pub struct Limits {
    pub timeout: Duration,
    pub max_rss_kib: Option<u64>,
    pub pin_cpu: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Exit {
    Code(i32),
    Signal(i32),
    Timeout,
    /// Killed by the watchdog because the resident set exceeded the cap.
    RssCap,
}

#[derive(Clone, Debug)]
pub struct ChildRun {
    pub exit: Exit,
    pub wall: Duration,
    pub stdout: String,
    pub stderr: String,
    /// Part of the output was not kept: the middle of a stream longer than [`CAPTURE_LIMIT`] (head and tail are
    /// kept, see `output_dropped_bytes`) or the end of a stream that was still open when the grace period ended
    /// (`capture_incomplete`).
    pub output_truncated: bool,
    /// Peak resident set of the child in KiB: the maximum `VmHWM` of `/proc/<pid>/status` over the watchdog's polls
    /// (never `wait4`'s `ru_maxrss`, see the module docs). `None` when no poll reached the process.
    pub max_rss_kib: Option<u64>,
    /// Number of `/proc/<pid>/status` polls that returned a value (0 <=> `max_rss_kib` is `None`).
    pub rss_samples: u32,
    pub cpu_us: Option<u64>,
    /// Bytes of the middle of stdout + stderr that were dropped (0 when everything fitted).
    pub output_dropped_bytes: u64,
    /// A pipe was still open when the grace period after the leader's exit ended (a detached grandchild holds it):
    /// the output is what had been read by then.
    pub capture_incomplete: bool,
    /// `process`: `max_rss_kib` is the leader's own `VmHWM`. `tree`: the leader had descendants (a wrapper that does
    /// not `exec` the tool) and the number is the maximum `VmHWM` over the leader and its descendants; the RSS cap
    /// was applied to that maximum too.
    pub rss_scope: &'static str,
}

impl ChildRun {
    pub fn wall_us(&self) -> f64 {
        self.wall.as_secs_f64() * 1e6
    }
    /// `measured_vmhwm` | `measured_tree_vmhwm` (a wrapper with descendants, see [`ChildRun::rss_scope`]) |
    /// `unmeasured` (the process ended before the first `/proc` poll): the `*_status` string that accompanies every
    /// reported peak RSS. The number is the maximum of polls, a LOWER BOUND of the true peak (the last poll interval
    /// before the exit is not seen).
    pub fn rss_status(&self) -> &'static str {
        if self.max_rss_kib.is_none() {
            "unmeasured"
        } else if self.rss_scope == "tree" {
            "measured_tree_vmhwm"
        } else {
            "measured_vmhwm"
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpawnError {
    NotFound(String),
    Other(String),
}

/// What the two reader threads and the caller share for one stream.
#[derive(Default)]
struct Capture {
    head: Vec<u8>,
    tail: Vec<u8>,
    dropped: u64,
    done: bool,
}

impl Capture {
    fn push(&mut self, mut chunk: &[u8]) {
        if self.head.len() < CAPTURE_HEAD {
            let take = chunk.len().min(CAPTURE_HEAD - self.head.len());
            self.head.extend_from_slice(&chunk[..take]);
            chunk = &chunk[take..];
        }
        if chunk.is_empty() {
            return;
        }
        self.tail.extend_from_slice(chunk);
        // amortised ring buffer: let the tail grow to twice its size, then drop the oldest half
        if self.tail.len() > 2 * CAPTURE_TAIL {
            let cut = self.tail.len() - CAPTURE_TAIL;
            self.tail.drain(..cut);
            self.dropped += cut as u64;
        }
    }

    /// The captured text (head, an explicit omission marker when bytes were dropped, tail) and the dropped bytes.
    fn finish(mut self) -> (Vec<u8>, u64) {
        if self.tail.len() > CAPTURE_TAIL {
            let cut = self.tail.len() - CAPTURE_TAIL;
            self.tail.drain(..cut);
            self.dropped += cut as u64;
        }
        if self.dropped == 0 {
            self.head.extend_from_slice(&self.tail);
            return (self.head, 0);
        }
        // do not start or end a kept part in the middle of a line when a line break is close by
        if let Some(i) = self.head.iter().rposition(|&b| b == b'\n') {
            if self.head.len() - i < 4096 {
                self.dropped += (self.head.len() - i - 1) as u64;
                self.head.truncate(i + 1);
            }
        }
        if let Some(i) = self.tail.iter().take(4096).position(|&b| b == b'\n') {
            self.dropped += (i + 1) as u64;
            self.tail.drain(..=i);
        }
        let mut out = self.head;
        if !out.ends_with(b"\n") {
            out.push(b'\n');
        }
        out.extend_from_slice(
            format!(
                "[... {} bytes of output omitted by the harness ...]\n",
                self.dropped
            )
            .as_bytes(),
        );
        out.extend_from_slice(&self.tail);
        (out, self.dropped)
    }
}

type SharedCapture = Arc<(Mutex<Capture>, Condvar)>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Read a pipe on a helper thread into a head + tail capture. The loop polls with a timeout instead of blocking
/// in `read`, so a pipe that a detached grandchild holds open for ever does not pin the thread: once `abandon` is
/// set the thread ends and closes its end.
fn spawn_reader<R: Read + AsRawFd + Send + 'static>(
    mut r: R,
    abandon: Arc<AtomicBool>,
    pin: Option<usize>,
) -> SharedCapture {
    let cap: SharedCapture = Arc::new((Mutex::new(Capture::default()), Condvar::new()));
    let shared = cap.clone();
    let spawned = std::thread::Builder::new()
        .name("bench-reader".to_string())
        .spawn(move || {
            avoid_cpu(pin);
            let mut chunk = [0u8; 16 * 1024];
            while !abandon.load(Ordering::Acquire) {
                let mut pfd = libc::pollfd {
                    fd: r.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                let n = unsafe { libc::poll(&mut pfd, 1, 50) };
                if n < 0 {
                    if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                        continue;
                    }
                    break;
                }
                if n == 0 {
                    continue;
                }
                match r.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(k) => lock(&shared.0).push(&chunk[..k]),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(_) => break,
                }
            }
            lock(&shared.0).done = true;
            shared.1.notify_all();
        });
    if spawned.is_err() {
        // no reader thread: the stream reads as empty and complete (the child may block on a full pipe and is
        // then ended by the watchdog)
        lock(&cap.0).done = true;
    }
    cap
}

/// `VmHWM` / `VmRSS` of a process in KiB (`None` for a field the kernel did not print: zombies, kernel threads).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VmStatus {
    pub hwm_kib: Option<u64>,
    pub rss_kib: Option<u64>,
}

/// Parse the text of `/proc/<pid>/status`.
pub fn parse_vm_status(text: &str) -> VmStatus {
    let mut v = VmStatus::default();
    for line in text.lines() {
        let kib = |rest: &str| {
            rest.split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
        };
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            v.hwm_kib = kib(rest);
        } else if let Some(rest) = line.strip_prefix("VmRSS:") {
            v.rss_kib = kib(rest);
        }
    }
    v
}

/// Memory status of `pid` from `/proc/<pid>/status`; `None` when the process is gone or unreadable.
pub fn read_vm_status(pid: u32) -> Option<VmStatus> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    Some(parse_vm_status(&text))
}

/// `/proc/self/cmdline` of the harness, read once. A child that has not yet switched to its own address space shows
/// exactly this (the argument area lives in the address space it still shares with, or has copied from, the parent).
fn own_cmdline() -> &'static [u8] {
    static OWN: OnceLock<Vec<u8>> = OnceLock::new();
    OWN.get_or_init(|| std::fs::read("/proc/self/cmdline").unwrap_or_default())
}

/// Memory status of a child, or `None` when the child's own address space was not provably the one read.
///
/// The status read is bracketed by two reads of `/proc/<pid>/cmdline`, which are served from the same address space as
/// `VmHWM`. The sample is accepted only when both are equal, non-empty, and differ from the harness's own command
/// line: the switch from the parent's to the child's address space happens once and in that direction, so a status
/// read before it is always preceded by a command line equal to the parent's, and a read after it is bracketed by the
/// child's. A child whose command line is identical to the harness's cannot be told apart and has no samples (an
/// honest `None`, never the parent's number).
pub fn sample_child(pid: u32) -> Option<VmStatus> {
    let own = own_cmdline();
    if own.is_empty() {
        return None;
    }
    let path = format!("/proc/{pid}/cmdline");
    let before = std::fs::read(&path).ok()?;
    if before.is_empty() || before == own {
        return None;
    }
    let status = read_vm_status(pid)?;
    let after = std::fs::read(&path).ok()?;
    (after == before).then_some(status)
}

/// One-minute load average, if available.
pub fn loadavg() -> Option<f64> {
    std::fs::read_to_string("/proc/loadavg")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// Number of `sched_setaffinity` calls that failed while pinning a child (the child then ran unpinned): a report
/// that says "pinned to CPU n" must check this.
static PIN_FAILURES: AtomicU32 = AtomicU32::new(0);

/// How many child starts could not be pinned to the requested CPU so far.
pub fn pin_failures() -> u32 {
    PIN_FAILURES.load(Ordering::Relaxed)
}

/// CPUs this process may run on (`sched_getaffinity`), ascending.
pub fn allowed_cpus() -> Vec<usize> {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set) != 0 {
            return Vec::new();
        }
        (0..libc::CPU_SETSIZE as usize)
            .filter(|c| libc::CPU_ISSET(*c, &set))
            .collect()
    }
}

/// `Ok` when `cpu` is a CPU this process is allowed to run on (a pin outside the caller's own mask would escape it,
/// an index at or above `CPU_SETSIZE` aborts inside `CPU_SET`).
pub fn validate_pin_cpu(cpu: usize) -> Result<(), String> {
    let allowed = allowed_cpus();
    if allowed.is_empty() {
        return Err("cannot read this process's CPU affinity mask".to_string());
    }
    if allowed.contains(&cpu) {
        Ok(())
    } else {
        let first = allowed.first().copied().unwrap_or(0);
        let last = allowed.last().copied().unwrap_or(0);
        Err(format!(
            "CPU {cpu} is not in this process's allowed set ({} CPUs, {first}..={last})",
            allowed.len()
        ))
    }
}

fn with_thread_affinity<T>(cpu: Option<usize>, f: impl FnOnce() -> T) -> T {
    let Some(cpu) = cpu else { return f() };
    if cpu >= libc::CPU_SETSIZE as usize {
        PIN_FAILURES.fetch_add(1, Ordering::Relaxed);
        return f();
    }
    unsafe {
        let mut old: libc::cpu_set_t = std::mem::zeroed();
        let have_old =
            libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut old) == 0;
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(cpu, &mut set);
        let pinned = libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0;
        if !pinned {
            PIN_FAILURES.fetch_add(1, Ordering::Relaxed);
        }
        let r = f();
        if pinned && have_old {
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &old);
        }
        r
    }
}

/// Keep the calling (helper) thread off `cpu`: the watchdog and the pipe readers must not compete with a pinned
/// child for its core.
fn avoid_cpu(cpu: Option<usize>) {
    let Some(cpu) = cpu else { return };
    if cpu >= libc::CPU_SETSIZE as usize {
        return;
    }
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set) != 0 {
            return;
        }
        libc::CPU_CLR(cpu, &mut set);
        let left = (0..libc::CPU_SETSIZE as usize)
            .filter(|c| libc::CPU_ISSET(*c, &set))
            .count();
        if left > 0 {
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
        }
    }
}

/// Polling interval of the watchdog: dense while the process is young (a start-up-dominated run lives a few
/// milliseconds and its peak is made early), sparser later (cheap for long runs).
fn poll_interval(age: Duration) -> Duration {
    if age < Duration::from_millis(20) {
        Duration::from_micros(250)
    } else if age < Duration::from_millis(500) {
        Duration::from_millis(1)
    } else {
        Duration::from_millis(4)
    }
}

fn kill_group(pid: i32) {
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
        libc::kill(pid, libc::SIGKILL);
    }
}

/// Direct children of `pid`'s main thread (`/proc/<pid>/task/<pid>/children`; empty when the kernel does not
/// provide the file, which degrades to "leader only").
fn children_of(pid: u32) -> Vec<u32> {
    std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children"))
        .map(|s| {
            s.split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Peak-memory sample of a child: the leader's `VmHWM` ([`sample_child`]) and, when `with_children`, the largest
/// `VmHWM` among its descendants (a wrapper script that does not `exec` the tool: the leader is a small shell and the
/// real work happens below it). Returns `(maximum KiB, a descendant was seen)`.
fn sample_tree(pid: u32, with_children: bool) -> Option<(u64, bool)> {
    let leader = sample_child(pid)?;
    let mut hwm = leader.hwm_kib.or(leader.rss_kib)?;
    let mut saw = false;
    if with_children {
        let mut frontier = vec![pid];
        let mut seen = 1usize;
        'levels: for _ in 0..3 {
            let mut next = Vec::new();
            for p in frontier {
                for c in children_of(p) {
                    if seen >= 32 {
                        break 'levels;
                    }
                    seen += 1;
                    next.push(c);
                    if let Some(k) = read_vm_status(c).and_then(|v| v.hwm_kib.or(v.rss_kib)) {
                        hwm = hwm.max(k);
                        saw = true;
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
    }
    Some((hwm, saw))
}

// ---------------------------------------------------------------------------------------------------------
// Process-group registry, scratch-directory registry, termination cleanup
// ---------------------------------------------------------------------------------------------------------

const MAX_LIVE_GROUPS: usize = 64;
/// Process-group ids of the children that are running right now (0 = free slot). Read by the signal handler, so
/// plain atomics.
static LIVE_GROUPS: [AtomicI32; MAX_LIVE_GROUPS] = [const { AtomicI32::new(0) }; MAX_LIVE_GROUPS];
/// Signal number that asked the harness to stop (0 = none yet).
static TERMINATING: AtomicI32 = AtomicI32::new(0);
/// Write end of the self-pipe that wakes the cleanup thread.
static WAKE_FD: AtomicI32 = AtomicI32::new(-1);
/// Scratch directories to remove when the harness is terminated by a signal (the `Drop` of [`TmpDir`] does not run).
static SCRATCH_DIRS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// The signal number that asked the harness to stop, if one did (the cleanup thread ends the process shortly after).
pub fn terminating_signal() -> Option<i32> {
    match TERMINATING.load(Ordering::Acquire) {
        0 => None,
        s => Some(s),
    }
}

fn register_group(pid: i32) -> Option<usize> {
    for (i, slot) in LIVE_GROUPS.iter().enumerate() {
        if slot
            .compare_exchange(0, pid, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return Some(i);
        }
    }
    None
}

fn unregister_group(slot: Option<usize>) {
    if let Some(i) = slot {
        LIVE_GROUPS[i].store(0, Ordering::Release);
    }
}

/// Number of child process groups currently registered (tests).
pub fn live_groups() -> usize {
    LIVE_GROUPS
        .iter()
        .filter(|s| s.load(Ordering::Acquire) > 0)
        .count()
}

fn kill_live_groups() {
    for slot in LIVE_GROUPS.iter() {
        let p = slot.load(Ordering::Acquire);
        if p > 0 {
            kill_group(p);
        }
    }
}

/// Signal handler: only atomics, `kill(2)` and `write(2)` (async-signal-safe). It kills every registered child
/// process group at once and wakes the cleanup thread; a second signal ends the process immediately.
extern "C" fn on_terminate(sig: libc::c_int) {
    if TERMINATING
        .compare_exchange(0, sig, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        unsafe { libc::_exit(128 + sig) };
    }
    kill_live_groups();
    let fd = WAKE_FD.load(Ordering::Acquire);
    if fd >= 0 {
        let b = [sig as u8];
        unsafe {
            libc::write(fd, b.as_ptr() as *const libc::c_void, 1);
        }
    }
}

/// Install SIGTERM / SIGINT / SIGHUP handling for a long suite run: the children's process groups are killed, every
/// registered scratch directory is removed and the process exits with `128 + signal` (143 / 130 / 129). Without it a
/// `timeout` or CI kill leaves the children running and the RAM-backed scratch directory behind. Idempotent. A
/// harness killed by SIGKILL cannot clean up; the children that are not timed repetitions additionally die with it
/// ([`orphan_guard`]).
pub fn install_termination_cleanup() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let mut fds = [0 as libc::c_int; 2];
        if libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) != 0 {
            return;
        }
        let rd = fds[0];
        WAKE_FD.store(fds[1], Ordering::Release);
        let spawned = std::thread::Builder::new()
            .name("bench-term".to_string())
            .spawn(move || {
                let mut b = [0u8; 1];
                loop {
                    let n = libc::read(rd, b.as_mut_ptr() as *mut libc::c_void, 1);
                    if n == 1 {
                        break;
                    }
                    if n < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
                    {
                        continue;
                    }
                    return;
                }
                let sig = TERMINATING.load(Ordering::Acquire);
                kill_live_groups();
                remove_scratch_dirs();
                eprintln!(
                    "rustspice bench: stopped by signal {sig}; child process groups killed, scratch directories removed"
                );
                libc::_exit(128 + sig);
            });
        if spawned.is_err() {
            WAKE_FD.store(-1, Ordering::Release);
            return;
        }
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = on_terminate as extern "C" fn(libc::c_int) as usize;
        sa.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut sa.sa_mask);
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::sigaction(sig, &sa, std::ptr::null_mut());
        }
    });
}

fn remove_scratch_dirs() {
    let dirs = std::mem::take(&mut *lock(&SCRATCH_DIRS));
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

thread_local! {
    /// Children started from this thread die with the harness (`PR_SET_PDEATHSIG`). Needs a `fork` + `pre_exec`
    /// start instead of `posix_spawn`, which costs 0.4-1.3 ms per start here (measured, 2-500 MiB parent): fine for
    /// correctness runs, far too much inside a timed repetition.
    static ORPHAN_GUARD: Cell<bool> = const { Cell::new(false) };
}

/// Restores the previous [`orphan_guard`] setting when dropped.
pub struct OrphanScope(bool);

impl Drop for OrphanScope {
    fn drop(&mut self) {
        ORPHAN_GUARD.with(|g| g.set(self.0));
    }
}

/// While the returned guard lives, children started by [`run_guarded`] from this thread get
/// `PR_SET_PDEATHSIG(SIGKILL)`: a harness that dies (SIGKILL, abort) leaves no orphans behind. Pass `false` around
/// timed repetitions, where the start-up cost of the `fork` path would bias the measurement.
pub fn orphan_guard(on: bool) -> OrphanScope {
    let prev = ORPHAN_GUARD.with(|g| g.replace(on));
    OrphanScope(prev)
}

fn orphan_guard_enabled() -> bool {
    ORPHAN_GUARD.with(|g| g.get())
}

/// Run `cmd` to completion under the limits. Stdin is closed; stdout/stderr are captured (bounded: head + tail).
pub fn run_guarded(cmd: &mut Command, lim: &Limits) -> Result<ChildRun, SpawnError> {
    // A harness that inherited SIGCHLD = SIG_IGN (the disposition survives `exec`) has its children reaped by the
    // kernel and `wait4` fails with ECHILD: every run would be lost. The default disposition is required.
    unsafe {
        libc::signal(libc::SIGCHLD, libc::SIG_DFL);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.process_group(0);
    if orphan_guard_enabled() {
        let parent = std::process::id() as libc::pid_t;
        unsafe {
            cmd.pre_exec(move || {
                // best effort: a failing prctl only means that the child can outlive a dying harness
                libc::prctl(
                    libc::PR_SET_PDEATHSIG,
                    libc::SIGKILL as libc::c_ulong,
                    0,
                    0,
                    0,
                );
                // the harness may have died before the call: PDEATHSIG is only delivered for a later death
                if libc::getppid() != parent {
                    libc::_exit(137);
                }
                Ok(())
            });
        }
    }
    let t0 = Instant::now();
    let mut child = with_thread_affinity(lim.pin_cpu, || cmd.spawn()).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            SpawnError::NotFound(e.to_string())
        } else {
            SpawnError::Other(e.to_string())
        }
    })?;
    let pid = child.id() as i32;
    let slot = register_group(pid);
    if terminating_signal().is_some() {
        // a termination signal arrived between the start and the registration: this child is not on the list
        kill_group(pid);
    }
    if let Some(kib) = lim.max_rss_kib {
        // Belt and braces against a single huge allocation: address-space cap at 4x the RSS cap (>= 8 GiB).
        let bytes = (kib.saturating_mul(4 * 1024)).max(8u64 << 30);
        let rl = libc::rlimit {
            rlim_cur: bytes,
            rlim_max: bytes,
        };
        unsafe {
            libc::prlimit(pid, libc::RLIMIT_AS, &rl, std::ptr::null_mut());
        }
    }
    let out = child.stdout.take();
    let err = child.stderr.take();
    let pin = lim.pin_cpu;
    let abandon = Arc::new(AtomicBool::new(false));
    let cap_o = out.map(|o| spawn_reader(o, abandon.clone(), pin));
    let cap_e = err.map(|e| spawn_reader(e, abandon.clone(), pin));

    let stop = Arc::new(AtomicBool::new(false));
    let reason = Arc::new(AtomicU8::new(0));
    let peak = Arc::new(AtomicU64::new(0));
    let samples = Arc::new(AtomicU32::new(0));
    let tree = Arc::new(AtomicBool::new(false));
    // first sample right away from this thread: a child that lives a millisecond or two is still caught
    if let Some((hwm, _)) = sample_tree(pid as u32, false) {
        peak.fetch_max(hwm, Ordering::AcqRel);
        samples.fetch_add(1, Ordering::AcqRel);
    }
    let wd = {
        let (stop, reason, peak, samples, tree) = (
            stop.clone(),
            reason.clone(),
            peak.clone(),
            samples.clone(),
            tree.clone(),
        );
        let timeout = lim.timeout;
        let cap = lim.max_rss_kib;
        std::thread::spawn(move || {
            avoid_cpu(pin);
            let mut polls: u64 = 0;
            while !stop.load(Ordering::Acquire) {
                let age = t0.elapsed();
                if age >= timeout {
                    reason.store(1, Ordering::Release);
                    if !stop.load(Ordering::Acquire) {
                        kill_group(pid);
                    }
                    break;
                }
                // descendants are looked up on every poll once the process is past its first milliseconds and, while
                // it is young, on every fourth one (an extra read of /proc per poll is cheap but not free)
                let look = age >= Duration::from_millis(20) || polls.is_multiple_of(4);
                polls += 1;
                // `VmHWM` is monotone: the cap sees a spike even when it was over before this poll
                if let Some((hwm, saw)) = sample_tree(pid as u32, look) {
                    peak.fetch_max(hwm, Ordering::AcqRel);
                    samples.fetch_add(1, Ordering::AcqRel);
                    if saw {
                        tree.store(true, Ordering::Release);
                    }
                    if let Some(cap) = cap {
                        if hwm > cap {
                            reason.store(2, Ordering::Release);
                            if !stop.load(Ordering::Acquire) {
                                kill_group(pid);
                            }
                            break;
                        }
                    }
                }
                std::thread::sleep(poll_interval(age));
            }
        })
    };

    let mut status: libc::c_int = 0;
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    let mut wait_errno = 0;
    let waited = loop {
        let r = unsafe { libc::wait4(pid, &mut status, 0, &mut ru) };
        if r == pid {
            break true;
        }
        if r < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error();
            if errno == Some(libc::EINTR) {
                continue;
            }
            wait_errno = errno.unwrap_or(0);
        }
        break false;
    };
    let wall = t0.elapsed();
    stop.store(true, Ordering::Release);
    let _ = wd.join();
    // reap stragglers of the group (grandchildren holding the pipes); group only, the leader pid is gone
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    unregister_group(slot);
    drop(child);
    // Readers normally see end-of-file at once. A detached grandchild (`setsid`) can keep a pipe open: after the
    // grace period the capture is taken as it is and flagged, never silently emptied.
    let deadline = Instant::now() + READER_GRACE;
    let mut incomplete = false;
    for cap in [&cap_o, &cap_e].into_iter().flatten() {
        let mut g = lock(&cap.0);
        while !g.done {
            let now = Instant::now();
            if now >= deadline {
                incomplete = true;
                break;
            }
            g = cap
                .1
                .wait_timeout(g, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
    abandon.store(true, Ordering::Release);
    if !waited {
        return Err(SpawnError::Other(format!(
            "wait4 failed (errno {wait_errno})"
        )));
    }
    let take = |c: &Option<SharedCapture>| -> (Vec<u8>, u64) {
        c.as_ref().map_or((Vec::new(), 0), |c| {
            std::mem::take(&mut *lock(&c.0)).finish()
        })
    };
    let (ob, od) = take(&cap_o);
    let (eb, ed) = take(&cap_e);

    let exit = if libc::WIFEXITED(status) {
        Exit::Code(libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        match reason.load(Ordering::Acquire) {
            1 => Exit::Timeout,
            2 => Exit::RssCap,
            _ => Exit::Signal(libc::WTERMSIG(status)),
        }
    } else {
        Exit::Signal(0)
    };
    // `ru.ru_maxrss` is deliberately not used: it is floored by the harness's own high-water mark (module docs)
    let n_samples = samples.load(Ordering::Acquire);
    let max_rss = if n_samples > 0 {
        Some(peak.load(Ordering::Acquire))
    } else {
        None
    };
    let cpu_us = Some(
        (ru.ru_utime.tv_sec as u64 * 1_000_000 + ru.ru_utime.tv_usec as u64)
            + (ru.ru_stime.tv_sec as u64 * 1_000_000 + ru.ru_stime.tv_usec as u64),
    );
    Ok(ChildRun {
        exit,
        wall,
        stdout: String::from_utf8_lossy(&ob).to_string(),
        stderr: String::from_utf8_lossy(&eb).to_string(),
        output_truncated: od + ed > 0 || incomplete,
        max_rss_kib: max_rss,
        rss_samples: n_samples,
        cpu_us,
        output_dropped_bytes: od + ed,
        capture_incomplete: incomplete,
        rss_scope: if tree.load(Ordering::Acquire) {
            "tree"
        } else {
            "process"
        },
    })
}

/// Temporary directory on tmpfs (`/dev/shm`) when available, else the system temp dir; removed on drop, and by the
/// termination cleanup ([`install_termination_cleanup`]) when the process is stopped by a signal.
pub struct TmpDir {
    path: PathBuf,
}

impl TmpDir {
    pub fn new(tag: &str) -> std::io::Result<TmpDir> {
        let pid = std::process::id();
        let mut candidates = vec![PathBuf::from("/dev/shm")];
        candidates.push(std::env::temp_dir());
        let mut last = None;
        for base in candidates {
            let p = base.join(format!("rustspice-bench-{pid}-{tag}"));
            match std::fs::create_dir_all(&p) {
                Ok(()) => {
                    lock(&SCRATCH_DIRS).push(p.clone());
                    return Ok(TmpDir { path: p });
                }
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn on_tmpfs(&self) -> bool {
        self.path.starts_with("/dev/shm")
    }
    /// Free space (bytes) of the file system that holds the directory (`None` when it cannot be read).
    pub fn free_bytes(&self) -> Option<u64> {
        free_bytes(&self.path)
    }
}

/// Space available to unprivileged writers on the file system of `path`.
pub fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    Some((st.f_bavail as u64).saturating_mul(st.f_frsize as u64))
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        lock(&SCRATCH_DIRS).retain(|p| p != &self.path);
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// ---------------------------------------------------------------------------------------------------------
// RustSpice subprocess runner
// ---------------------------------------------------------------------------------------------------------

/// One analysis entry of the `--stats` file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalysisStat {
    pub analysis: String,
    pub analysis_us: Option<f64>,
    pub points: Option<u64>,
}

/// The `rustspice run --stats` JSON (unavailable quantities are `null`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunStats {
    pub parse_us: Option<f64>,
    pub compile_us: Option<f64>,
    pub solver_kind: Option<String>,
    pub solver_compile_us: Option<f64>,
    pub analysis: Option<String>,
    pub analysis_us: Option<f64>,
    pub write_us: Option<f64>,
    pub peak_rss_kib: Option<u64>,
    pub points: Option<u64>,
    pub status: Option<String>,
    #[serde(default)]
    pub analyses: Vec<AnalysisStat>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RsClass {
    Ok,
    /// Non-zero exit with a message on stderr, no panic: the simulator refused the deck.
    LoudError(String),
    Panic(String),
    Timeout,
}

#[derive(Clone, Debug)]
pub struct RsRun {
    pub child: ChildRun,
    pub class: RsClass,
    pub stats: Option<RunStats>,
    pub raw_path: PathBuf,
    pub stats_path: PathBuf,
    /// stderr carries a `panicked at` message although the process ended with a clean non-zero exit (a panic that
    /// was caught and turned into an error). Informational only: classification keys on exit code 101 / signals.
    pub caught_panic_noise: bool,
    /// The run ended with [`HARNESS_IO_EXIT`]: RustSpice could not WRITE its output (full or failing disk). An
    /// environment failure, not a verdict about the deck: the suite reports the case `unverified`.
    pub infra_failure: bool,
}

/// Longest message kept in a report (bytes). A `ParseError` echoes the offending token, and a mutant with a 100 KB
/// parenthesised expression produced a 100 KB error line that was then stored three times in the JSON report.
pub const MESSAGE_LIMIT: usize = 400;

/// `s` cut to [`MESSAGE_LIMIT`] bytes on a character boundary, with the size of the rest named (never silently).
pub fn clip_message(s: &str) -> String {
    if s.len() <= MESSAGE_LIMIT {
        return s.to_string();
    }
    let mut end = MESSAGE_LIMIT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{} ... [{} more bytes]", &s[..end], s.len() - end)
}

/// Most informative stderr line: a panic / allocation / stack-overflow message if present, else an `Error` line,
/// else the last non-empty line (clipped to [`MESSAGE_LIMIT`]).
fn last_message_line(stderr: &str) -> String {
    let lines: Vec<&str> = stderr
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    for pat in [
        "panicked at",
        "memory allocation of",
        "stack overflow",
        "fatal runtime error",
        "Error",
    ] {
        if let Some(l) = lines.iter().find(|l| l.contains(pat)) {
            return clip_message(l);
        }
    }
    lines
        .last()
        .map_or("(no message on stderr)".to_string(), |l| clip_message(l))
}

/// Message of a clean refusal: the `Error...` line the CLI prints (a caught panic may have printed noise before
/// it), else the most informative line.
fn refusal_message(stderr: &str) -> String {
    stderr
        .lines()
        .map(|l| l.trim())
        .find(|l| l.starts_with("Error"))
        .map_or_else(|| last_message_line(stderr), clip_message)
}

/// Exit code 101, SIGABRT/SIGSEGV/..., the RSS cap and the watchdog are panics/timeouts. A clean non-zero exit
/// is a loud refusal even when stderr carries a caught `panicked at` message (see
/// [`RsRun::caught_panic_noise`]). [`HARNESS_IO_EXIT`] is neither: it is an I/O failure of the harness's own output
/// (the message says so, [`RsRun::infra_failure`] flags it, the suite makes the case `unverified`).
pub fn classify_exit(child: &ChildRun, raw_exists: bool) -> RsClass {
    match &child.exit {
        Exit::Timeout => RsClass::Timeout,
        // not a Rust panic or abort: the harness ended the process (the status stays `panic`, a robustness violation,
        // but the message and `RsRun::termination` say who pulled the trigger)
        Exit::RssCap => RsClass::Panic(
            "killed by the harness at the RSS cap (unbounded memory growth; not a Rust panic or abort)"
                .to_string(),
        ),
        Exit::Signal(s) => {
            let name = match *s {
                libc::SIGABRT => "SIGABRT",
                libc::SIGSEGV => "SIGSEGV",
                libc::SIGBUS => "SIGBUS",
                libc::SIGILL => "SIGILL",
                libc::SIGFPE => "SIGFPE",
                libc::SIGKILL => "SIGKILL (external, possibly the OOM killer)",
                _ => "signal",
            };
            RsClass::Panic(format!(
                "terminated by {name} ({s}): {}",
                last_message_line(&child.stderr)
            ))
        }
        Exit::Code(101) => RsClass::Panic(format!(
            "panic (exit 101): {}",
            last_message_line(&child.stderr)
        )),
        Exit::Code(HARNESS_IO_EXIT) => RsClass::LoudError(format!(
            "harness I/O failure (exit {HARNESS_IO_EXIT}): {}",
            refusal_message(&child.stderr)
        )),
        Exit::Code(c) if *c != 0 => {
            RsClass::LoudError(format!("exit {c}: {}", refusal_message(&child.stderr)))
        }
        Exit::Code(_) => {
            if raw_exists {
                RsClass::Ok
            } else {
                RsClass::LoudError("exit 0 but no rawfile was written".to_string())
            }
        }
    }
}

/// Name of a signal number (`SIGABRT`, ...), `SIG<n>` for the rest.
pub fn signal_name(s: i32) -> String {
    match s {
        libc::SIGABRT => "SIGABRT".to_string(),
        libc::SIGSEGV => "SIGSEGV".to_string(),
        libc::SIGBUS => "SIGBUS".to_string(),
        libc::SIGILL => "SIGILL".to_string(),
        libc::SIGFPE => "SIGFPE".to_string(),
        libc::SIGKILL => "SIGKILL".to_string(),
        libc::SIGTERM => "SIGTERM".to_string(),
        libc::SIGXFSZ => "SIGXFSZ".to_string(),
        libc::SIGPIPE => "SIGPIPE".to_string(),
        other => format!("SIG{other}"),
    }
}

/// Run `<exe> run <deck> --raw <dir>/<tag>.raw --stats <dir>/<tag>.json`.
pub fn run(
    exe: &Path,
    deck: &Path,
    dir: &Path,
    tag: &str,
    lim: &Limits,
) -> Result<RsRun, SpawnError> {
    run_in(exe, deck, dir, tag, lim, None)
}

/// [`run`] with the child's working directory set (relative `.include` targets resolve against it).
pub fn run_in(
    exe: &Path,
    deck: &Path,
    dir: &Path,
    tag: &str,
    lim: &Limits,
    cwd: Option<&Path>,
) -> Result<RsRun, SpawnError> {
    let raw_path = dir.join(format!("{tag}.raw"));
    let stats_path = dir.join(format!("{tag}.json"));
    let _ = std::fs::remove_file(&raw_path);
    let _ = std::fs::remove_file(&stats_path);
    let mut cmd = Command::new(exe);
    cmd.arg("run")
        .arg(deck)
        .arg("--raw")
        .arg(&raw_path)
        .arg("--stats")
        .arg(&stats_path);
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let child = run_guarded(&mut cmd, lim)?;
    let class = classify_exit(&child, raw_path.exists());
    let infra_failure = child.exit == Exit::Code(HARNESS_IO_EXIT);
    let caught_panic_noise = matches!(class, RsClass::LoudError(_))
        && !infra_failure
        && child.stderr.contains("panicked at");
    let stats = std::fs::read_to_string(&stats_path)
        .ok()
        .and_then(|s| serde_json::from_str::<RunStats>(&s).ok())
        .map(|mut st| {
            // the stats file repeats the refusal text (and with it any 100 KB token the error echoes)
            st.status = st.status.map(|m| clip_message(&m));
            st
        });
    Ok(RsRun {
        child,
        class,
        stats,
        raw_path,
        stats_path,
        caught_panic_noise,
        infra_failure,
    })
}

impl RsRun {
    /// How the process ended, in one machine-readable token: `exit_<n>` | `rust_panic_exit_101` |
    /// `signal_<NAME>` (the process crashed or was killed from outside) | `killed_by_harness_rss_cap` |
    /// `killed_by_harness_timeout` | `harness_io_failure`. An RSS-cap kill is a memory-growth finding, a signal a
    /// crash: the two must not be confused in a report.
    pub fn termination(&self) -> String {
        match &self.child.exit {
            Exit::Timeout => "killed_by_harness_timeout".to_string(),
            Exit::RssCap => "killed_by_harness_rss_cap".to_string(),
            Exit::Signal(s) => format!("signal_{}", signal_name(*s)),
            Exit::Code(101) => "rust_panic_exit_101".to_string(),
            Exit::Code(HARNESS_IO_EXIT) => "harness_io_failure".to_string(),
            Exit::Code(c) => format!("exit_{c}"),
        }
    }

    /// Peak resident set of the RustSpice child in KiB: the larger of the watchdog's `VmHWM` polls and the
    /// process's own `--stats peak_rss_kib` (its `VmHWM` read just before it exits, i.e. the final sample no poll
    /// can take, because a zombie has no `Vm*` fields). `None` when neither exists.
    pub fn peak_rss_kib(&self) -> Option<u64> {
        let own = self.stats.as_ref().and_then(|s| s.peak_rss_kib);
        match (self.child.max_rss_kib, own) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }
    /// `measured_vmhwm` (polls and the process's own final reading), `vmhwm_poll_only`, `own_stats_only` or
    /// `unmeasured`.
    pub fn rss_status(&self) -> &'static str {
        let own = self
            .stats
            .as_ref()
            .is_some_and(|s| s.peak_rss_kib.is_some());
        match (self.child.max_rss_kib.is_some(), own) {
            (true, true) => "measured_vmhwm",
            (true, false) => "vmhwm_poll_only",
            (false, true) => "own_stats_only",
            (false, false) => "unmeasured",
        }
    }
}

pub fn load_plots(run: &RsRun) -> Result<Vec<Plot>, String> {
    rawfile::read_file(&run.raw_path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Make this process's resident high-water mark at least `mib` MiB; the returned buffer must stay alive. The
    /// volatile writes keep the optimiser from deleting an otherwise unused allocation.
    fn fatten(mib: usize) -> Vec<u8> {
        let mut v = vec![0u8; mib << 20];
        for i in (0..v.len()).step_by(4096) {
            unsafe { std::ptr::write_volatile(&mut v[i], 1) };
        }
        let p = std::hint::black_box(v.as_ptr());
        assert!(!p.is_null());
        v
    }

    fn limits(secs: u64) -> Limits {
        Limits {
            timeout: Duration::from_secs(secs),
            max_rss_kib: Some(2 * 1024 * 1024),
            pin_cpu: None,
        }
    }

    #[test]
    fn captures_output_and_exit_code() {
        let mut c = Command::new("sh");
        c.args(["-c", "echo out; echo err >&2; exit 3"]);
        let r = run_guarded(&mut c, &limits(10)).unwrap();
        assert_eq!(r.exit, Exit::Code(3));
        assert_eq!(r.stdout.trim(), "out");
        assert_eq!(r.stderr.trim(), "err");
        // a few-millisecond child may end before the first poll: then there is no number (never 0)
        assert!(r.max_rss_kib.is_none_or(|k| k > 0));
    }

    #[test]
    fn watchdog_kills_process_group_on_timeout() {
        let mut c = Command::new("sh");
        c.args(["-c", "sleep 30 & sleep 30; echo never"]);
        let t = Instant::now();
        let r = run_guarded(&mut c, &limits(1)).unwrap();
        assert_eq!(r.exit, Exit::Timeout);
        assert!(
            t.elapsed() < Duration::from_secs(10),
            "took {:?}",
            t.elapsed()
        );
    }

    #[test]
    fn signals_are_classified_as_panics() {
        let mut c = Command::new("sh");
        c.args(["-c", "kill -ABRT $$"]);
        let r = run_guarded(&mut c, &limits(10)).unwrap();
        assert_eq!(r.exit, Exit::Signal(libc::SIGABRT));
        assert!(matches!(classify_exit(&r, true), RsClass::Panic(_)));
        let mut c = Command::new("sh");
        c.args(["-c", "echo 'thread main panicked at x' >&2; exit 101"]);
        let r = run_guarded(&mut c, &limits(10)).unwrap();
        assert!(matches!(classify_exit(&r, false), RsClass::Panic(_)));
        let mut c = Command::new("sh");
        c.args(["-c", "echo 'Error: nope' >&2; exit 1"]);
        let r = run_guarded(&mut c, &limits(10)).unwrap();
        assert_eq!(
            classify_exit(&r, false),
            RsClass::LoudError("exit 1: Error: nope".to_string())
        );
    }

    #[test]
    fn a_caught_panic_with_a_clean_exit_is_a_refusal_not_a_panic() {
        // faer panic noise, then the CLI's own `Error: SingularMatrix` and exit 1 (tag caught_panic_noise)
        let mut c = Command::new("sh");
        c.args([
            "-c",
            "echo \"thread 'main' panicked at faer lu.rs: explicit panic\" >&2; echo 'Error: SingularMatrix' >&2; exit 1",
        ]);
        let r = run_guarded(&mut c, &limits(10)).unwrap();
        assert_eq!(
            classify_exit(&r, false),
            RsClass::LoudError("exit 1: Error: SingularMatrix".to_string())
        );
        // exit 101 is the uncaught panic and stays one
        let mut c = Command::new("sh");
        c.args(["-c", "echo \"thread 'main' panicked at x\" >&2; exit 101"]);
        let r = run_guarded(&mut c, &limits(10)).unwrap();
        assert!(matches!(classify_exit(&r, false), RsClass::Panic(_)));
    }

    #[test]
    fn rss_cap_kills_memory_hog() {
        let mut c = Command::new("python3");
        c.args(["-c", "import time\nx = bytearray(600*1024*1024)\nfor i in range(0, len(x), 4096): x[i] = 1\ntime.sleep(20)"]);
        let lim = Limits {
            timeout: Duration::from_secs(20),
            max_rss_kib: Some(200 * 1024),
            pin_cpu: None,
        };
        let r = run_guarded(&mut c, &lim).unwrap();
        assert_eq!(r.exit, Exit::RssCap, "{r:?}");
    }

    #[test]
    fn pinning_applies_to_child_and_is_restored() {
        let allowed = || unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set);
            (0..libc::CPU_SETSIZE as usize)
                .filter(|c| libc::CPU_ISSET(*c, &set))
                .collect::<Vec<_>>()
        };
        let before = allowed();
        let cpu = before[before.len() / 2];
        let mut c = Command::new("sh");
        c.args(["-c", "grep Cpus_allowed_list /proc/self/status"]);
        let lim = Limits {
            timeout: Duration::from_secs(10),
            max_rss_kib: None,
            pin_cpu: Some(cpu),
        };
        let r = run_guarded(&mut c, &lim).unwrap();
        assert!(
            r.stdout.trim().ends_with(&format!("\t{cpu}")),
            "child affinity `{}` should be exactly cpu {cpu}",
            r.stdout.trim()
        );
        assert_eq!(
            allowed(),
            before,
            "calling thread affinity must be restored"
        );
    }

    #[test]
    fn long_refusal_messages_are_clipped_on_a_char_boundary_and_say_so() {
        assert_eq!(clip_message("short"), "short");
        let long = format!(
            "Error: ParseError {{ message: \"{}\" }}",
            "(".repeat(100_000)
        );
        let c = clip_message(&long);
        assert!(c.len() < MESSAGE_LIMIT + 40, "{}", c.len());
        assert!(c.starts_with("Error: ParseError"));
        assert!(c.ends_with("more bytes]"), "{c}");
        // multi-byte characters straddling the limit never split
        let wide = "\u{e9}".repeat(MESSAGE_LIMIT);
        let c = clip_message(&wide);
        assert!(c.is_char_boundary(c.len()) && c.contains("more bytes]"));
        // the classifier applies it to a refusal and to a panic line
        let child = ChildRun {
            exit: Exit::Code(1),
            wall: Duration::from_millis(1),
            stdout: String::new(),
            stderr: format!("{long}\n"),
            output_truncated: false,
            max_rss_kib: None,
            rss_samples: 0,
            cpu_us: None,
            output_dropped_bytes: 0,
            capture_incomplete: false,
            rss_scope: "process",
        };
        match classify_exit(&child, false) {
            RsClass::LoudError(m) => assert!(m.len() < MESSAGE_LIMIT + 60, "{}", m.len()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn vm_status_parser_reads_hwm_and_rss() {
        let t = "Name:\tsh\nVmPeak:\t  9000 kB\nVmHWM:\t    1234 kB\nVmRSS:\t     987 kB\nThreads:\t1\n";
        assert_eq!(
            parse_vm_status(t),
            VmStatus {
                hwm_kib: Some(1234),
                rss_kib: Some(987)
            }
        );
        // a zombie / kernel thread has no Vm* lines: nothing is invented
        assert_eq!(
            parse_vm_status("Name:\tx\nState:\tZ (zombie)\n"),
            VmStatus::default()
        );
        // malformed numbers are ignored, not read as 0
        assert_eq!(parse_vm_status("VmHWM:\t  abc kB\n").hwm_kib, None);
        // this very process has both
        let me = read_vm_status(std::process::id()).unwrap();
        assert!(me.hwm_kib.unwrap() >= me.rss_kib.unwrap());
    }

    #[test]
    fn a_status_read_is_trusted_only_for_the_childs_own_address_space() {
        // this very process looks exactly like "a child that still shares the parent's address space": rejected
        assert_eq!(sample_child(std::process::id()), None);
        // a process that is gone has no sample either
        assert_eq!(sample_child(4_000_000), None);
        // a real child with its own command line is accepted once it has exec'd
        let mut ch = Command::new("sleep").arg("5").spawn().unwrap();
        let mut got = None;
        for _ in 0..2000 {
            got = sample_child(ch.id());
            if got.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let _ = ch.kill();
        let _ = ch.wait();
        let v = got.expect("an exec'd `sleep` must be sampled");
        assert!(
            v.hwm_kib.unwrap() > 0 && v.hwm_kib.unwrap() < 64 * 1024,
            "{v:?}"
        );
    }

    /// Regression for the D8 finding: `wait4`'s `ru_maxrss` of a vfork-style child is floored by the parent's own
    /// high-water mark, so a tiny child of a fat harness reported the harness's footprint (identical numbers for
    /// RustSpice and both ngspice solvers). The number must be the child's own.
    #[test]
    fn a_big_parent_footprint_does_not_inflate_a_tiny_childs_peak_rss() {
        // grow this process's high-water mark by ~320 MiB and keep it resident
        let fat = fatten(320);
        let mine = read_vm_status(std::process::id()).unwrap();
        assert!(
            mine.hwm_kib.unwrap() >= 300 * 1024,
            "test setup: parent HWM {mine:?}"
        );
        let mut c = Command::new("sh");
        c.args(["-c", "sleep 0.3"]);
        let r = run_guarded(&mut c, &limits(20)).unwrap();
        assert_eq!(r.exit, Exit::Code(0));
        let kib = r.max_rss_kib.expect("a 300 ms child is polled many times");
        assert!(r.rss_samples >= 5, "{r:?}");
        assert_eq!(r.rss_status(), "measured_vmhwm");
        assert!(
            kib > 0 && kib < 64 * 1024,
            "the child's peak RSS must not carry the parent's 320 MiB: {kib} KiB"
        );
        // `wait4` itself would have said otherwise (informational: kernel/libc dependent, not asserted)
        let mut std_cmd = Command::new("true");
        std_cmd.stdout(Stdio::null());
        let mut ch = std_cmd.spawn().unwrap();
        let mut st: libc::c_int = 0;
        let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
        unsafe { libc::wait4(ch.id() as i32, &mut st, 0, &mut ru) };
        let _ = ch.try_wait();
        eprintln!(
            "info: wait4 ru_maxrss of a trivial child of this {} MiB parent = {} KiB",
            mine.hwm_kib.unwrap() / 1024,
            ru.ru_maxrss
        );
        drop(fat);
    }

    /// Regression found by the first pinned timing run: right after `spawn` returns the child can still own the
    /// parent's address space (the vfork-style `exec` wakes the parent before it switches the child to the new
    /// one), so a `/proc/<pid>/status` read in that window reports the PARENT's `VmHWM`. Pinned to the parent's CPU
    /// the window is wide, and a single such sample poisons the maximum.
    #[test]
    fn pinned_children_of_a_fat_parent_never_report_the_parents_footprint() {
        let fat = fatten(256);
        let parent = read_vm_status(std::process::id()).unwrap();
        assert!(
            parent.hwm_kib.unwrap() >= 250 * 1024,
            "test setup: parent {parent:?}"
        );
        let cpu = unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set);
            (0..libc::CPU_SETSIZE as usize)
                .find(|c| libc::CPU_ISSET(*c, &set))
                .unwrap()
        };
        let mut measured = 0;
        for i in 0..60 {
            let mut c = Command::new("sh");
            c.args(["-c", "sleep 0.01"]).current_dir("/");
            let lim = Limits {
                timeout: Duration::from_secs(20),
                max_rss_kib: None,
                pin_cpu: Some(cpu),
            };
            let r = run_guarded(&mut c, &lim).unwrap();
            if let Some(k) = r.max_rss_kib {
                measured += 1;
                assert!(
                    k < 64 * 1024,
                    "run {i}: a `sh` child reported {} MiB (the fat parent holds 256 MiB): {r:?}",
                    k / 1024
                );
            }
        }
        assert!(
            measured >= 30,
            "the poller should catch a 10 ms child most of the time: {measured}/60"
        );
        drop(fat);
    }

    #[test]
    fn a_hungry_child_is_measured_at_its_own_size() {
        let mut c = Command::new("python3");
        c.args(["-c", "import time\nx = bytearray(150*1024*1024)\nfor i in range(0, len(x), 4096): x[i] = 1\ntime.sleep(0.5)"]);
        let r = run_guarded(&mut c, &limits(30)).unwrap();
        assert_eq!(r.exit, Exit::Code(0), "{r:?}");
        let mib = r.max_rss_kib.unwrap() / 1024;
        assert!(
            (140..400).contains(&mib),
            "expected ~150-200 MiB, got {mib}"
        );
    }

    #[test]
    fn a_child_that_exits_before_any_poll_has_no_number_not_zero() {
        // `true` is gone within a millisecond: depending on scheduling the poller may or may not catch it; the
        // contract is that the answer is a positive number or None, never Some(0)
        for _ in 0..20 {
            let mut c = Command::new("true");
            let r = run_guarded(&mut c, &limits(10)).unwrap();
            match r.max_rss_kib {
                Some(k) => {
                    assert!(k > 0);
                    assert!(r.rss_samples > 0);
                    assert_eq!(r.rss_status(), "measured_vmhwm");
                }
                None => {
                    assert_eq!(r.rss_samples, 0);
                    assert_eq!(r.rss_status(), "unmeasured");
                }
            }
        }
    }

    #[test]
    fn rustspice_peak_rss_combines_polls_and_its_own_final_reading() {
        let mk = |poll: Option<u64>, own: Option<u64>| RsRun {
            child: ChildRun {
                exit: Exit::Code(0),
                wall: Duration::from_millis(1),
                stdout: String::new(),
                stderr: String::new(),
                output_truncated: false,
                max_rss_kib: poll,
                rss_samples: u32::from(poll.is_some()),
                cpu_us: None,
                output_dropped_bytes: 0,
                capture_incomplete: false,
                rss_scope: "process",
            },
            class: RsClass::Ok,
            stats: Some(RunStats {
                peak_rss_kib: own,
                ..Default::default()
            }),
            raw_path: PathBuf::new(),
            stats_path: PathBuf::new(),
            caught_panic_noise: false,
            infra_failure: false,
        };
        assert_eq!(mk(Some(10), Some(30)).peak_rss_kib(), Some(30));
        assert_eq!(mk(Some(50), Some(30)).peak_rss_kib(), Some(50));
        assert_eq!(mk(None, Some(30)).peak_rss_kib(), Some(30));
        assert_eq!(mk(Some(10), None).peak_rss_kib(), Some(10));
        assert_eq!(mk(None, None).peak_rss_kib(), None);
        assert_eq!(mk(None, None).rss_status(), "unmeasured");
        assert_eq!(mk(Some(1), Some(2)).rss_status(), "measured_vmhwm");
        assert_eq!(mk(None, Some(2)).rss_status(), "own_stats_only");
        assert_eq!(mk(Some(1), None).rss_status(), "vmhwm_poll_only");
    }

    #[test]
    fn missing_binary_is_reported() {
        let mut c = Command::new("/nonexistent/definitely-not-here");
        assert!(matches!(
            run_guarded(&mut c, &limits(5)),
            Err(SpawnError::NotFound(_))
        ));
    }

    #[test]
    fn output_is_bounded() {
        let mut c = Command::new("sh");
        c.args(["-c", "head -c 3000000 /dev/zero | tr '\\0' x"]);
        let r = run_guarded(&mut c, &limits(20)).unwrap();
        assert_eq!(r.exit, Exit::Code(0));
        // head + tail + one omission marker line, never more than the limit plus the marker
        assert!(
            r.stdout.len() <= CAPTURE_LIMIT + 100 && r.stdout.len() >= CAPTURE_LIMIT / 2,
            "{}",
            r.stdout.len()
        );
        assert!(r.output_truncated);
        assert!(
            r.output_dropped_bytes > 2_000_000,
            "{}",
            r.output_dropped_bytes
        );
        assert!(r.stdout.contains("bytes of output omitted by the harness"));
        assert!(!r.capture_incomplete);
    }

    /// ER-process-02: ngspice prints `Total analysis time` at the END of a long output; the old capture kept the first
    /// 256 KiB and lost it without a word.
    #[test]
    fn a_long_output_keeps_its_head_and_its_tail() {
        let mut c = Command::new("sh");
        c.args([
            "-c",
            "echo HEAD-LINE; head -c 3000000 /dev/zero | tr '\\0' x; echo; echo 'Total analysis time (seconds) = 0.5'; echo >&2 ERR-TAIL",
        ]);
        let r = run_guarded(&mut c, &limits(20)).unwrap();
        assert!(r.stdout.starts_with("HEAD-LINE\n"), "{:?}", &r.stdout[..20]);
        assert!(
            r.stdout
                .trim_end()
                .ends_with("Total analysis time (seconds) = 0.5"),
            "the tail must survive: {:?}",
            &r.stdout[r.stdout.len() - 60..]
        );
        assert!(r.output_truncated && r.output_dropped_bytes > 2_000_000);
        assert_eq!(r.stderr.trim(), "ERR-TAIL");
        // an output that fits is returned verbatim, without a marker
        let mut c = Command::new("sh");
        c.args(["-c", "echo one; echo two"]);
        let r = run_guarded(&mut c, &limits(10)).unwrap();
        assert_eq!(r.stdout, "one\ntwo\n");
        assert!(!r.output_truncated && r.output_dropped_bytes == 0);
    }

    /// ER-process-10: a grandchild that detached with `setsid` keeps the pipes open. The harness used to wait 2 s per
    /// stream and then return EMPTY output; now it returns what was read, quickly, and says that it is incomplete.
    #[test]
    fn a_detached_grandchild_holding_the_pipes_does_not_empty_the_capture() {
        let mut c = Command::new("sh");
        c.args([
            "-c",
            "echo hello-out; echo hello-err >&2; setsid sleep 4 & sleep 0.2; exit 1",
        ]);
        let t = Instant::now();
        let r = run_guarded(&mut c, &limits(20)).unwrap();
        assert_eq!(r.exit, Exit::Code(1));
        assert!(r.stdout.contains("hello-out"), "{r:?}");
        assert!(r.stderr.contains("hello-err"), "{r:?}");
        assert!(r.capture_incomplete && r.output_truncated, "{r:?}");
        assert!(
            t.elapsed() < Duration::from_millis(2500),
            "took {:?}",
            t.elapsed()
        );
    }

    /// ER-process-07: a wrapper that does not `exec` is a small shell; the real peak is below it.
    #[test]
    fn a_wrapper_that_does_not_exec_is_measured_as_a_process_tree() {
        let hog = "import time\nx = bytearray(150*1024*1024)\nfor i in range(0, len(x), 4096): x[i] = 1\ntime.sleep(0.6)";
        let script = format!("python3 -c '{hog}'; echo done");
        let mut c = Command::new("sh");
        c.args(["-c", &script]);
        let r = run_guarded(&mut c, &limits(30)).unwrap();
        assert_eq!(r.exit, Exit::Code(0), "{r:?}");
        assert_eq!(r.rss_scope, "tree", "{r:?}");
        assert_eq!(r.rss_status(), "measured_tree_vmhwm");
        let mib = r.max_rss_kib.unwrap() / 1024;
        assert!(
            (140..400).contains(&mib),
            "the shell is ~1 MiB, the python child 150+ MiB: got {mib} MiB"
        );
        // a tool that is started directly stays `process`
        let mut c = Command::new("sleep");
        c.arg("0.3");
        let r = run_guarded(&mut c, &limits(10)).unwrap();
        assert_eq!(r.rss_scope, "process");
    }

    #[test]
    fn the_rss_cap_applies_to_the_descendants_of_a_wrapper_too() {
        let hog = "import time\nx = bytearray(600*1024*1024)\nfor i in range(0, len(x), 4096): x[i] = 1\ntime.sleep(20)";
        let script = format!("python3 -c '{hog}'; echo done");
        let mut c = Command::new("sh");
        c.args(["-c", &script]);
        let lim = Limits {
            timeout: Duration::from_secs(30),
            max_rss_kib: Some(200 * 1024),
            pin_cpu: None,
        };
        let t = Instant::now();
        let r = run_guarded(&mut c, &lim).unwrap();
        assert_eq!(r.exit, Exit::RssCap, "{r:?}");
        assert!(t.elapsed() < Duration::from_secs(15), "{:?}", t.elapsed());
    }

    /// ER-process-06: an index at or above CPU_SETSIZE aborted the harness inside `CPU_SET`; a CPU outside the
    /// allowed set was ignored while the report said "pinned".
    #[test]
    fn pin_cpu_is_validated_and_a_failed_pin_is_counted_not_hidden() {
        let allowed = allowed_cpus();
        assert!(!allowed.is_empty());
        assert!(validate_pin_cpu(allowed[0]).is_ok());
        assert!(validate_pin_cpu(5000).unwrap_err().contains("not in"));
        let outside = (0..1024).find(|c| !allowed.contains(c));
        if let Some(c) = outside {
            assert!(validate_pin_cpu(c).is_err());
        }
        let before = pin_failures();
        let mut c = Command::new("true");
        let lim = Limits {
            timeout: Duration::from_secs(10),
            max_rss_kib: None,
            pin_cpu: Some(5000),
        };
        let r = run_guarded(&mut c, &lim).unwrap();
        assert_eq!(r.exit, Exit::Code(0));
        assert!(pin_failures() > before, "the failed pin must be counted");
    }

    #[test]
    fn a_harness_io_failure_is_not_a_refusal_and_a_cap_kill_is_not_a_crash() {
        let mk = |exit: Exit, stderr: &str| ChildRun {
            exit,
            wall: Duration::from_millis(1),
            stdout: String::new(),
            stderr: stderr.to_string(),
            output_truncated: false,
            max_rss_kib: None,
            rss_samples: 0,
            cpu_us: None,
            output_dropped_bytes: 0,
            capture_incomplete: false,
            rss_scope: "process",
        };
        let io = classify_exit(
            &mk(
                Exit::Code(HARNESS_IO_EXIT),
                "Error: cannot write rawfile x.raw: No space left on device\n",
            ),
            false,
        );
        assert!(
            matches!(&io, RsClass::LoudError(m) if m.starts_with("harness I/O failure")),
            "{io:?}"
        );
        // the same stderr under any other exit code stays an ordinary refusal
        assert!(
            matches!(classify_exit(&mk(Exit::Code(1), "Error: cannot write rawfile\n"), false), RsClass::LoudError(m) if m.starts_with("exit 1"))
        );
        let rs = |exit: Exit| RsRun {
            child: mk(exit, ""),
            class: RsClass::Ok,
            stats: None,
            raw_path: PathBuf::new(),
            stats_path: PathBuf::new(),
            caught_panic_noise: false,
            infra_failure: false,
        };
        assert_eq!(rs(Exit::RssCap).termination(), "killed_by_harness_rss_cap");
        assert_eq!(
            rs(Exit::Signal(libc::SIGABRT)).termination(),
            "signal_SIGABRT"
        );
        assert_eq!(rs(Exit::Code(101)).termination(), "rust_panic_exit_101");
        assert_eq!(rs(Exit::Code(1)).termination(), "exit_1");
        assert_eq!(rs(Exit::Timeout).termination(), "killed_by_harness_timeout");
        match classify_exit(&mk(Exit::RssCap, ""), false) {
            RsClass::Panic(m) => {
                assert!(
                    m.contains("by the harness") && !m.contains("SIGABRT"),
                    "{m}"
                )
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_live_group_registry_hands_out_and_frees_slots() {
        let a = register_group(2_000_000_001).unwrap();
        let b = register_group(2_000_000_002).unwrap();
        assert_ne!(a, b);
        assert!(live_groups() >= 2);
        unregister_group(Some(a));
        unregister_group(Some(b));
        unregister_group(None);
        // a freed slot is reused
        let c = register_group(2_000_000_003).unwrap();
        unregister_group(Some(c));
    }

    #[test]
    fn orphan_guard_scopes_nest_and_restore() {
        assert!(!orphan_guard_enabled());
        {
            let _a = orphan_guard(true);
            assert!(orphan_guard_enabled());
            {
                let _b = orphan_guard(false);
                assert!(!orphan_guard_enabled());
            }
            assert!(orphan_guard_enabled());
            // a child started under the guard still runs and is classified like any other
            let mut c = Command::new("sh");
            c.args(["-c", "echo guarded; exit 2"]);
            let r = run_guarded(&mut c, &limits(10)).unwrap();
            assert_eq!(r.exit, Exit::Code(2));
            assert_eq!(r.stdout.trim(), "guarded");
        }
        assert!(!orphan_guard_enabled());
    }

    #[test]
    fn tmpdir_is_removed() {
        let p;
        {
            let t = TmpDir::new("unit").unwrap();
            p = t.path().to_path_buf();
            assert!(p.exists());
        }
        assert!(!p.exists());
    }
}
