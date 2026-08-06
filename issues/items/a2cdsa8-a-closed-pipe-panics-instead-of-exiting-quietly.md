# a closed pipe panics instead of exiting quietly

## Summary
`trck list --all | head` — the most ordinary pipeline anyone will type against this tool — makes
the binary panic once the reader closes the pipe before the writer is done:

```
thread 'main' panicked at library/std/src/io/stdio.rs:1165:9:
failed printing to stdout: Broken pipe (os error 32)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

Rust's `println!` unwraps the write, and on a closed stdout that is an `Err`. It needs enough
output to fill the pipe buffer before the reader goes away, which is why small trackers and
short outputs hide it; `list --all` over this repo's 219 issues reproduces it every time.

This is exactly the failure the crate's lints were written to prevent — `unwrap`, `expect`,
`panic` and `unsafe` are all denied on the grounds that a bad input must produce a diagnostic
rather than a stack trace — and the standard-library macro slipped underneath them. The Python
engine handles it no better in kind but far better in degree: `Exception ignored while flushing
sys.stdout: BrokenPipeError`, one line, no backtrace, no invitation to set `RUST_BACKTRACE`.

A closed pipe is not an error. `head` closing early is the shell working as designed, and the
right response is to stop writing and exit, silently and successfully.

## Acceptance criteria
- [ ] A closed stdout ends the process quietly — no panic, no backtrace, no `RUST_BACKTRACE`
      advice — for every verb that writes a lot: `list`, `tree`, `deps`, `html`, `show`.
- [ ] The exit status is the conventional one for the case, and `trck list | head -1` returns
      what `head` asked for.
- [ ] The fix is in the one place output leaves the engine, not sprinkled per verb; a verb
      added later must not have to remember this.
- [ ] A test covers it, closing the reader before the writer finishes rather than relying on
      output happening to exceed the pipe buffer.
