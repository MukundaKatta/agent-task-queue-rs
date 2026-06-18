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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Queued and waiting to be picked up by [`TaskQueue::pop_highest`].
    Pending,
    /// Currently being executed.
    Running,
    /// Completed successfully.
    Done,
    /// Failed; the wrapped string describes the reason.
    Failed(String),
    /// Deliberately skipped (will be removed by [`TaskQueue::clear_done`]).
    Skipped,
}

impl TaskStatus {
    /// Returns `true` if the status is final and the task will not run again.
    ///
    /// Terminal statuses are [`TaskStatus::Done`], [`TaskStatus::Failed`] and
    /// [`TaskStatus::Skipped`]. [`TaskStatus::Pending`] and
    /// [`TaskStatus::Running`] are non-terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Done | TaskStatus::Failed(_) | TaskStatus::Skipped
        )
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
    /// Create a new, empty queue.
    pub fn new() -> Self {
        Self::default()
    }

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
    pub fn push_with_meta(
        &mut self,
        description: impl Into<String>,
        priority: i32,
        meta: Value,
    ) -> usize {
        let id = self.push(description, priority);
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.metadata = Some(meta);
        }
        id
    }

    /// Index of the highest-priority pending task.
    ///
    /// Among tasks with equal priority, the one inserted **first** wins
    /// (stable / FIFO ordering), so the queue is deterministic.
    fn highest_pending_idx(&self) -> Option<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.status == TaskStatus::Pending)
            // `max_by_key` keeps the last maximum, so iterate in reverse over
            // the id (insertion order) to break ties in favour of the earliest
            // inserted task while still selecting the highest priority.
            .max_by_key(|(_, t)| (t.priority, std::cmp::Reverse(t.id)))
            .map(|(i, _)| i)
    }

    /// Remove and return the highest-priority pending task.
    ///
    /// Returns `None` when there are no pending tasks. Tasks with a status
    /// other than [`TaskStatus::Pending`] are never returned. Ties in priority
    /// are broken in favour of the task inserted first (FIFO).
    pub fn pop_highest(&mut self) -> Option<Task> {
        let idx = self.highest_pending_idx()?;
        Some(self.tasks.remove(idx))
    }

    /// Return a reference to the highest-priority pending task **without**
    /// removing it. Uses the same ordering rules as [`Self::pop_highest`].
    pub fn peek_highest(&self) -> Option<&Task> {
        let idx = self.highest_pending_idx()?;
        self.tasks.get(idx)
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

    /// Mark a task by id as skipped.
    ///
    /// Skipped tasks are terminal and are removed by [`Self::clear_done`].
    /// Returns `true` if a task with the given id existed, `false` otherwise.
    pub fn mark_skipped(&mut self, id: usize) -> bool {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.status = TaskStatus::Skipped;
            true
        } else {
            false
        }
    }

    /// Borrow a task by id, if it exists.
    pub fn get(&self, id: usize) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Mutably borrow a task by id, if it exists.
    pub fn get_mut(&mut self, id: usize) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    /// Number of tasks still in the [`TaskStatus::Pending`] state.
    pub fn pending_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .count()
    }

    /// Returns `true` if the queue holds no tasks at all (in any status).
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Total number of tasks in the queue, regardless of status.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Borrow every task in the queue, in insertion order.
    pub fn all(&self) -> &[Task] {
        &self.tasks
    }

    /// Collect references to all tasks currently in the given status.
    pub fn by_status(&self, status: &TaskStatus) -> Vec<&Task> {
        self.tasks.iter().filter(|t| &t.status == status).collect()
    }

    /// Remove all tasks that have reached a "finished" state, i.e.
    /// [`TaskStatus::Done`] or [`TaskStatus::Skipped`]. Failed tasks are kept
    /// so callers can inspect or retry them.
    pub fn clear_done(&mut self) {
        self.tasks
            .retain(|t| !matches!(t.status, TaskStatus::Done | TaskStatus::Skipped));
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

    #[test]
    fn equal_priority_is_fifo() {
        let mut q = TaskQueue::new();
        q.push("first", 5);
        q.push("second", 5);
        q.push("third", 5);
        // Same priority: the earliest inserted task is returned first.
        assert_eq!(q.pop_highest().unwrap().description, "first");
        assert_eq!(q.pop_highest().unwrap().description, "second");
        assert_eq!(q.pop_highest().unwrap().description, "third");
    }

    #[test]
    fn peek_does_not_remove() {
        let mut q = TaskQueue::new();
        q.push("low", 1);
        q.push("high", 10);
        let peeked = q.peek_highest().unwrap();
        assert_eq!(peeked.description, "high");
        // Peeking must not change the queue length.
        assert_eq!(q.len(), 2);
        // And a subsequent pop returns the same task we peeked.
        assert_eq!(q.pop_highest().unwrap().description, "high");
    }

    #[test]
    fn peek_empty_returns_none() {
        let q = TaskQueue::new();
        assert!(q.peek_highest().is_none());
    }

    #[test]
    fn mark_skipped_sets_status_and_reports_existence() {
        let mut q = TaskQueue::new();
        let id = q.push("skip-me", 1);
        assert!(q.mark_skipped(id));
        assert_eq!(q.get(id).unwrap().status, TaskStatus::Skipped);
        // Unknown id returns false and changes nothing.
        assert!(!q.mark_skipped(9999));
    }

    #[test]
    fn skipped_tasks_are_not_popped() {
        let mut q = TaskQueue::new();
        let high = q.push("high-but-skipped", 10);
        q.push("normal", 1);
        q.mark_skipped(high);
        // The skipped task must be ignored even though it has higher priority.
        assert_eq!(q.pop_highest().unwrap().description, "normal");
    }

    #[test]
    fn clear_done_removes_skipped() {
        let mut q = TaskQueue::new();
        let id = q.push("skipped", 1);
        q.push("pending", 1);
        q.mark_skipped(id);
        q.clear_done();
        assert_eq!(q.len(), 1);
        assert_eq!(q.all()[0].description, "pending");
    }

    #[test]
    fn clear_done_keeps_failed() {
        let mut q = TaskQueue::new();
        let id = q.push("boom", 1);
        q.mark_failed(id, "explosion");
        q.clear_done();
        // Failed tasks survive so they can be inspected or retried.
        assert_eq!(q.len(), 1);
        assert!(matches!(q.all()[0].status, TaskStatus::Failed(_)));
    }

    #[test]
    fn get_and_get_mut() {
        let mut q = TaskQueue::new();
        let id = q.push("editable", 1);
        assert_eq!(q.get(id).unwrap().description, "editable");
        assert!(q.get(424242).is_none());

        // Mutate through get_mut.
        q.get_mut(id).unwrap().priority = 99;
        assert_eq!(q.get(id).unwrap().priority, 99);
        assert!(q.get_mut(7777).is_none());
    }

    #[test]
    fn pending_count_ignores_non_pending() {
        let mut q = TaskQueue::new();
        let a = q.push("a", 1);
        let b = q.push("b", 2);
        q.push("c", 3);
        q.mark_running(a);
        q.mark_done(b);
        // Only "c" is still pending.
        assert_eq!(q.pending_count(), 1);
    }

    #[test]
    fn negative_priority_ordering() {
        let mut q = TaskQueue::new();
        q.push("very-low", -10);
        q.push("zero", 0);
        q.push("low", -1);
        // Highest numeric priority wins, including across negatives.
        assert_eq!(q.pop_highest().unwrap().description, "zero");
        assert_eq!(q.pop_highest().unwrap().description, "low");
        assert_eq!(q.pop_highest().unwrap().description, "very-low");
    }
}
