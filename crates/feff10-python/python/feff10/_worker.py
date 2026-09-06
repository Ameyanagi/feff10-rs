"""Private single-stage worker, launched by the native pipeline wrapper."""

from feff10._feff10 import _worker_init

if __name__ == "__main__":
    _worker_init()  # Runs the requested stage and exits without returning.
    raise SystemExit("feff10 worker: no stage was specified")
