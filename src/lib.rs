/*!
agent-task-queue: priority queue of agent tasks with status tracking.

```rust
use agent_task_queue::{TaskQueue, TaskStatus};

let mut q = TaskQueue::new();
q.push("send email", 5);
q.push("write report", 10);
let t = q.pop_highest().unwrap();
assert_eq!(t.description, "write report");
```
*/

use serde_json::Value;

/// Status of a task in the queue.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Done,
    Failed(String),
    Skipped,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Done | TaskStatus::Failed(_) | TaskStatus::Skipped)
    }
}

/// A single task entry.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: usize,
    pub description: String,
    pub priority: i32,
    pub status: TaskStatus,
    pub metadata: Option<Value>,
}

/// Priority task queue. Higher priority value = higher precedence.
#[derive(Debug, Default)]
pub struct TaskQueue {
    tasks: Vec<Task>,
    next_id: usize,
}

impl TaskQueue {
    pub fn new() -> Self { Self::default() }

    /// Add a task with the given priority. Returns the task id.
    pub fn push(&mut self, description: impl Into<String>, priority: i32) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task {
            id,
            description: description.into(),
            priority,
            status: TaskStatus::Pending,
            metadata: None,
        });
        id
    }

    /// Add a task with metadata.
    pub fn push_with_meta(&mut self, description: impl Into<String>, priority: i32, meta: Value) -> usize {
        let id = self.push(description, priority);
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.metadata = Some(meta);
        }
        id
    }

    /// Remove and return the highest-priority pending task.
    pub fn pop_highest(&mut self) -> Option<Task> {
        let idx = self.tasks.iter().enumerate()
            .filter(|(_, t)| t.status == TaskStatus::Pending)
            .max_by_key(|(_, t)| t.priority)
            .map(|(i, _)| i)?;
        Some(self.tasks.remove(idx))
    }

    /// Mark a task by id as running.
    pub fn mark_running(&mut self, id: usize) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.status = TaskStatus::Running;
        }
    }

    /// Mark a task by id as done.
    pub fn mark_done(&mut self, id: usize) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.status = TaskStatus::Done;
        }
    }

    /// Mark a task by id as failed.
    pub fn mark_failed(&mut self, id: usize, reason: impl Into<String>) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.status = TaskStatus::Failed(reason.into());
        }
    }

    pub fn pending_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.status == TaskStatus::Pending).count()
    }

    pub fn is_empty(&self) -> bool { self.tasks.is_empty() }
    pub fn len(&self) -> usize { self.tasks.len() }

    pub fn all(&self) -> &[Task] { &self.tasks }

    pub fn by_status(&self, status: &TaskStatus) -> Vec<&Task> {
        self.tasks.iter().filter(|t| &t.status == status).collect()
    }

    pub fn clear_done(&mut self) {
        self.tasks.retain(|t| !matches!(t.status, TaskStatus::Done | TaskStatus::Skipped));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn push_and_pop() {
        let mut q = TaskQueue::new();
        q.push("task", 1);
        assert!(!q.is_empty());
        let t = q.pop_highest().unwrap();
        assert_eq!(t.description, "task");
    }

    #[test]
    fn pop_returns_highest_priority() {
        let mut q = TaskQueue::new();
        q.push("low", 1);
        q.push("high", 10);
        q.push("mid", 5);
        let t = q.pop_highest().unwrap();
        assert_eq!(t.description, "high");
    }

    #[test]
    fn pop_empty_returns_none() {
        let mut q = TaskQueue::new();
        assert!(q.pop_highest().is_none());
    }

    #[test]
    fn ids_are_sequential() {
        let mut q = TaskQueue::new();
        let a = q.push("a", 1);
        let b = q.push("b", 1);
        assert_ne!(a, b);
    }

    #[test]
    fn mark_running() {
        let mut q = TaskQueue::new();
        let id = q.push("work", 1);
        // Don't pop — mark_running on tasks in queue
        q.mark_running(id);
        assert_eq!(q.by_status(&TaskStatus::Running).len(), 1);
    }

    #[test]
    fn mark_done() {
        let mut q = TaskQueue::new();
        let id = q.push("work", 1);
        q.mark_done(id);
        assert_eq!(q.by_status(&TaskStatus::Done).len(), 1);
    }

    #[test]
    fn mark_failed() {
        let mut q = TaskQueue::new();
        let id = q.push("work", 1);
        q.mark_failed(id, "timeout");
        let task = &q.all()[0];
        assert!(matches!(&task.status, TaskStatus::Failed(r) if r == "timeout"));
    }

    #[test]
    fn pending_count() {
        let mut q = TaskQueue::new();
        q.push("a", 1);
        q.push("b", 2);
        assert_eq!(q.pending_count(), 2);
        q.pop_highest();
        assert_eq!(q.pending_count(), 1);
    }

    #[test]
    fn clear_done_removes_finished() {
        let mut q = TaskQueue::new();
        let id = q.push("done-task", 1);
        q.push("pending-task", 1);
        q.mark_done(id);
        q.clear_done();
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn push_with_meta() {
        let mut q = TaskQueue::new();
        q.push_with_meta("tagged", 1, json!({"source": "user"}));
        let t = &q.all()[0];
        assert!(t.metadata.is_some());
    }

    #[test]
    fn terminal_statuses() {
        assert!(TaskStatus::Done.is_terminal());
        assert!(TaskStatus::Failed("err".into()).is_terminal());
        assert!(TaskStatus::Skipped.is_terminal());
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::Running.is_terminal());
    }

    #[test]
    fn pop_skips_non_pending() {
        let mut q = TaskQueue::new();
        let id_a = q.push("a", 5);
        q.push("b", 1);
        q.mark_running(id_a);
        // pop should return "b" since "a" is Running
        let t = q.pop_highest().unwrap();
        assert_eq!(t.description, "b");
    }
}
