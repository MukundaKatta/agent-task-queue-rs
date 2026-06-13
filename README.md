# agent-task-queue

A small, dependency-light Rust library providing a **priority queue of agent tasks with status tracking**. It is aimed at LLM/agent runtimes that need to schedule work by priority and observe each task as it moves through its lifecycle.

## Features

- **Priority scheduling** — `pop_highest()` always returns the highest-priority *pending* task (higher `priority` value = higher precedence).
- **Status tracking** — every task carries a `TaskStatus`: `Pending`, `Running`, `Done`, `Failed(reason)`, or `Skipped`.
- **Terminal-state helper** — `TaskStatus::is_terminal()` distinguishes finished tasks (`Done`, `Failed`, `Skipped`) from in-flight ones.
- **Optional metadata** — attach arbitrary JSON (`serde_json::Value`) to a task via `push_with_meta`.
- **Queue introspection** — count pending tasks, filter by status, list all tasks, and prune completed ones.

## Installation

Add it to your `Cargo.toml`:

```toml
[dependencies]
agent-task-queue = "0.1"
```

## Usage

```rust
use agent_task_queue::{TaskQueue, TaskStatus};

let mut q = TaskQueue::new();
q.push("send email", 5);
q.push("write report", 10);

// Highest priority comes out first.
let t = q.pop_highest().unwrap();
assert_eq!(t.description, "write report");
```

### Tracking task lifecycle

```rust
use agent_task_queue::{TaskQueue, TaskStatus};

let mut q = TaskQueue::new();
let id = q.push("run analysis", 3);

q.mark_running(id);
// ... do the work ...
q.mark_done(id);

assert_eq!(q.by_status(&TaskStatus::Done).len(), 1);

// Remove finished/skipped tasks from the queue.
q.clear_done();
```

### Attaching metadata

```rust
use agent_task_queue::TaskQueue;
use serde_json::json;

let mut q = TaskQueue::new();
q.push_with_meta("tagged task", 1, json!({ "source": "user" }));
```

## API overview

| Method | Description |
| --- | --- |
| `TaskQueue::new()` | Create an empty queue. |
| `push(description, priority)` | Add a pending task; returns its `id`. |
| `push_with_meta(description, priority, meta)` | Add a task with attached JSON metadata. |
| `pop_highest()` | Remove and return the highest-priority pending task. |
| `mark_running(id)` / `mark_done(id)` / `mark_failed(id, reason)` | Transition a task's status. |
| `pending_count()` | Number of pending tasks. |
| `by_status(&status)` | Borrow all tasks with a given status. |
| `all()` | Slice of all tasks. |
| `len()` / `is_empty()` | Total task count helpers. |
| `clear_done()` | Drop `Done` and `Skipped` tasks. |

## Tech stack

- **Language:** Rust (edition 2021)
- **Dependencies:** [`serde_json`](https://crates.io/crates/serde_json) (for optional task metadata)

## Development

```bash
cargo build
cargo test
```

## License

Licensed under the [MIT](LICENSE) license.
