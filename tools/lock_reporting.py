"""Best-effort lock diagnostics; never part of the lock's granting policy.

A kernel lock-table read can take minutes under load. Discovery runs once in
its own daemon thread; callers use the latest completed snapshot or report it
as pending. Threads neither hold queue mutexes nor grant resources.
"""
from __future__ import annotations

import io
import os
import threading
import time


class Snapshot:
    """One background observation at a time, refreshed only when requested."""

    def __init__(self, scan, refresh_s=60):
        self._scan = scan
        self._closed = False
        self._state = threading.Lock()
        self._refresh_s = refresh_s
        self._value = None
        self._finished = 0.0
        self._thread = None
        self.value()

    def value(self):
        with self._state:
            if self._closed:
                return self._value
            if self._thread is None or (not self._thread.is_alive()
                                       and time.monotonic() - self._finished >= self._refresh_s):
                self._thread = threading.Thread(target=self._run, daemon=True)
                self._thread.start()
            return self._value

    def _run(self):
        try:
            value = self._scan()
        except Exception:
            # Discovery cannot change a kernel lock verdict. An unavailable
            # diagnostic stays unknown; it must never masquerade as no holder.
            value = None
        with self._state:
            if not self._closed:
                self._value = value
            self._finished = time.monotonic()

    def close(self, wait=False):
        """Stop further discovery/publication; a read already in progress may finish.

        The scanner holds no queue or resource lock. Its file read closes
        naturally; callers must not wait for an uninterruptible diagnostic.
        Tests that own a deliberately blocked scan release it before `wait`.
        """
        with self._state:
            self._closed = True
        if wait and self._thread is not None:
            self._thread.join()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()


def bounded_output(write, out, timeout_s=1):
    """Publish a complete diagnostic or a pending notice within the deadline."""
    result = io.StringIO()
    done = threading.Event()
    errors = []

    def run():
        try:
            write(result)
        except Exception as exc:
            errors.append(exc)
        finally:
            done.set()

    threading.Thread(target=run, daemon=True).start()
    if done.wait(timeout_s):
        if errors:
            raise errors[0]
        out.write(result.getvalue())
    else:
        # Partial output is not a complete census. Do not publish it as one.
        print("heavy lock status: holder diagnostics pending (scan still running)", file=out)
    out.flush()


def duration(seconds):
    """Elapsed time for a person, with unknown durations kept explicit."""
    if seconds is None:
        return "?"
    if seconds < 60:
        return f"{seconds:.0f} s"
    minutes = int(seconds // 60)
    return f"{minutes} min" if minutes < 60 else f"{minutes // 60} h {minutes % 60:02d} min"


def still_line(lock, waited, modes, metadata, command_chars):
    """Format the latest observation, keeping pending discovery explicit."""
    prefix = f"still waiting for {lock} after {duration(waited)}"
    if modes is None:
        return f"{prefix}; holder diagnostics pending"
    holders = "; ".join(
        f"pid {pid} ({mode}, up {duration(metadata.process_age(pid))}): "
        f"{' '.join(metadata.cmdline(pid).split())[:command_chars]}"
        for pid, mode in modes if pid != os.getpid())
    return f"{prefix}; held by {holders or 'nobody'}"


def wait_line(lock, description, ahead, urgent, gates, modes):
    """Format queue policy supplied by the caller, without discovering holders."""
    holders = ", ".join(f"{pid} ({mode})" for pid, mode in modes
                        if pid != os.getpid()) or "none"
    return (f"waiting for {lock} held by pid {holders}; {ahead} waiter(s) ahead, "
            f"{urgent} of them priority, {gates} gate(s); this waiter: {description}")
