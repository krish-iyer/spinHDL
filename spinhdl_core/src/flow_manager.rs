use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Planned,
    Running,
    Succeeded,
    Failed,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub parent_id: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SubTask {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            parent_id: String::new(),
            status: TaskStatus::Planned,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtasks: Vec<SubTask>,
}

impl Task {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            status: TaskStatus::Planned,
            error: None,
            subtasks: Vec::new(),
        }
    }

    pub fn add_subtask(&mut self, mut st: SubTask) {
        st.parent_id = self.id.clone();
        self.subtasks.push(st);
    }

    pub fn set_status(&mut self, st: TaskStatus) {
        self.status = st;
        if st != TaskStatus::Failed {
            self.error = None;
        }
    }

    pub fn set_error(&mut self, msg: String) {
        self.status = TaskStatus::Failed;
        self.error = Some(msg);
    }

    pub fn set_subtask_status(&mut self, sub_id: &str, st: TaskStatus) {
        if let Some(stask) = self.subtasks.iter_mut().find(|s| s.id == sub_id) {
            stask.status = st;
            if st != TaskStatus::Failed {
                stask.error = None;
            }
        }
    }

    pub fn set_subtask_error(&mut self, sub_id: &str, msg: String) {
        if let Some(stask) = self.subtasks.iter_mut().find(|s| s.id == sub_id) {
            stask.status = TaskStatus::Failed;
            stask.error = Some(msg);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlowState {
    #[serde(default)]
    pub tasks: Vec<Task>,
}

#[derive(Debug, Default)]
pub struct FlowManager {
    state: FlowState,
}

impl FlowManager {
    pub fn new() -> Self {
        Self {
            state: FlowState::default(),
        }
    }

    pub fn save_to_toml(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let p = path.as_ref();
        if let Some(dir) = p.parent() {
            fs::create_dir_all(dir)?;
        }
        let toml = toml::to_string_pretty(&self.state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let tmp = p.with_extension("toml.tmp");
        fs::write(&tmp, toml)?;
        fs::rename(tmp, p)?;
        Ok(())
    }

    pub fn upsert_task(&mut self, task: Task) {
        if let Some(existing) = self.state.tasks.iter_mut().find(|t| t.id == task.id) {
            *existing = task;
        } else {
            self.state.tasks.push(task);
        }
    }

    pub fn tasks(&self) -> &[Task] {
        &self.state.tasks
    }

    pub fn task_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.state.tasks.iter_mut().find(|t| t.id == id)
    }
}
