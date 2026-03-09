//! TASKS.md parser — extracts structured task data from the markdown file.

use std::path::Path;

use regex::Regex;

/// A single task parsed from TASKS.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub phase: u32,
    /// `' '` = pending, `'~'` = in-progress, `'x'` = completed.
    pub status: char,
    pub description: String,
    pub blockers: Vec<String>,
}

/// Parse TASKS.md into a list of [`Task`] objects.
pub fn parse_tasks(path: &Path) -> anyhow::Result<Vec<Task>> {
    let content = std::fs::read_to_string(path)?;

    let phase_re = Regex::new(r"^##\s+Phase\s+(?P<phase>\d+):")?;
    let task_re =
        Regex::new(r"^-\s+\[(?P<status>[ ~x])\]\s+\*\*(?P<id>\d+\.\d+)\*\*\s+(?P<desc>.+?)$")?;
    let blocker_re = Regex::new(r"\[blocked by:\s*(?P<blockers>[^\]]+)\]")?;

    let mut tasks = Vec::new();
    let mut current_phase: u32 = 0;

    for line in content.lines() {
        if let Some(caps) = phase_re.captures(line) {
            current_phase = caps["phase"].parse()?;
            continue;
        }

        let trimmed = line.trim();
        let Some(caps) = task_re.captures(trimmed) else {
            continue;
        };

        let status = caps["status"].chars().next().unwrap();
        let mut desc = caps["desc"].to_string();
        let mut blockers = Vec::new();

        if let Some(blocker_caps) = blocker_re.captures(&desc) {
            let raw = &blocker_caps["blockers"];
            blockers = raw.split(',').map(|b| b.trim().to_string()).collect();
            let start = blocker_caps.get(0).unwrap().start();
            desc = desc[..start].trim().trim_end_matches('→').trim().to_string();
        }

        tasks.push(Task {
            id: caps["id"].to_string(),
            phase: current_phase,
            status,
            description: desc,
            blockers,
        });
    }

    Ok(tasks)
}

/// Check whether a single blocker string is resolved.
fn blocker_resolved(blocker: &str, tasks: &[Task]) -> bool {
    // Phase-level blocker (e.g. "Phase 3")
    let phase_re = Regex::new(r"^Phase\s+(\d+)$").unwrap();
    if let Some(caps) = phase_re.captures(blocker) {
        let phase_num: u32 = caps[1].parse().unwrap();
        return tasks
            .iter()
            .filter(|t| t.phase == phase_num)
            .all(|t| t.status == 'x');
    }

    // Task-level blocker (e.g. "3.6")
    for t in tasks {
        if t.id == blocker {
            return t.status == 'x';
        }
    }

    // Unknown blocker — treat as unresolved
    false
}

/// Find the next actionable task.
///
/// Prefers resuming an in-progress task, then picks the first pending task
/// whose blockers are all resolved. If `target_phase` is set, only considers
/// tasks in that phase.
pub fn find_next_task(tasks: &[Task], target_phase: Option<u32>) -> Option<&Task> {
    let candidates: Vec<&Task> = match target_phase {
        Some(p) => tasks.iter().filter(|t| t.phase == p).collect(),
        None => tasks.iter().collect(),
    };

    // Resume in-progress tasks first
    for t in &candidates {
        if t.status == '~' {
            return Some(t);
        }
    }

    // Find first pending task with resolved blockers
    for t in &candidates {
        if t.status != ' ' {
            continue;
        }
        if t.blockers.iter().all(|b| blocker_resolved(b, tasks)) {
            return Some(t);
        }
    }

    None
}

/// Check whether all relevant tasks are completed.
pub fn check_completion(tasks: &[Task], target_phase: Option<u32>) -> bool {
    let candidates: Vec<&Task> = match target_phase {
        Some(p) => tasks.iter().filter(|t| t.phase == p).collect(),
        None => tasks.iter().collect(),
    };
    candidates.iter().all(|t| t.status == 'x')
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn write_tasks(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parse_basic_tasks() {
        let f = write_tasks(
            "\
# TASKS.md

## Phase 1: Foundation

- [x] **1.1** Setup project structure
- [ ] **1.2** Add database layer [blocked by: 1.1]
- [~] **1.3** Implement API endpoints [blocked by: 1.1, 1.2]

## Phase 2: Frontend

- [ ] **2.1** Create UI components [blocked by: Phase 1]
",
        );

        let tasks = parse_tasks(f.path()).unwrap();
        assert_eq!(tasks.len(), 4);

        assert_eq!(tasks[0].id, "1.1");
        assert_eq!(tasks[0].phase, 1);
        assert_eq!(tasks[0].status, 'x');
        assert_eq!(tasks[0].description, "Setup project structure");
        assert!(tasks[0].blockers.is_empty());

        assert_eq!(tasks[1].id, "1.2");
        assert_eq!(tasks[1].status, ' ');
        assert_eq!(tasks[1].blockers, vec!["1.1"]);

        assert_eq!(tasks[2].id, "1.3");
        assert_eq!(tasks[2].status, '~');
        assert_eq!(tasks[2].blockers, vec!["1.1", "1.2"]);

        assert_eq!(tasks[3].id, "2.1");
        assert_eq!(tasks[3].phase, 2);
        assert_eq!(tasks[3].blockers, vec!["Phase 1"]);
    }

    #[test]
    fn find_next_prefers_in_progress() {
        let f = write_tasks(
            "\
## Phase 1:

- [ ] **1.1** First task
- [~] **1.2** In progress task
- [ ] **1.3** Another pending
",
        );

        let tasks = parse_tasks(f.path()).unwrap();
        let next = find_next_task(&tasks, None).unwrap();
        assert_eq!(next.id, "1.2");
    }

    #[test]
    fn find_next_skips_blocked() {
        let f = write_tasks(
            "\
## Phase 1:

- [ ] **1.1** Blocked task [blocked by: 1.2]
- [ ] **1.2** Unblocked task
",
        );

        let tasks = parse_tasks(f.path()).unwrap();
        let next = find_next_task(&tasks, None).unwrap();
        assert_eq!(next.id, "1.2");
    }

    #[test]
    fn find_next_respects_phase_filter() {
        let f = write_tasks(
            "\
## Phase 1:

- [ ] **1.1** Phase 1 task

## Phase 2:

- [ ] **2.1** Phase 2 task
",
        );

        let tasks = parse_tasks(f.path()).unwrap();
        let next = find_next_task(&tasks, Some(2)).unwrap();
        assert_eq!(next.id, "2.1");
    }

    #[test]
    fn find_next_returns_none_when_all_blocked() {
        let f = write_tasks(
            "\
## Phase 1:

- [x] **1.1** Done
- [ ] **1.2** Blocked [blocked by: 9.9]
",
        );

        let tasks = parse_tasks(f.path()).unwrap();
        // 9.9 doesn't exist, treated as unresolved
        let next = find_next_task(&tasks, None);
        assert!(next.is_none());
    }

    #[test]
    fn blocker_resolved_task_level() {
        let tasks = vec![
            Task {
                id: "1.1".into(),
                phase: 1,
                status: 'x',
                description: "Done".into(),
                blockers: vec![],
            },
            Task {
                id: "1.2".into(),
                phase: 1,
                status: ' ',
                description: "Pending".into(),
                blockers: vec!["1.1".into()],
            },
        ];

        assert!(blocker_resolved("1.1", &tasks));
        assert!(!blocker_resolved("1.2", &tasks));
        assert!(!blocker_resolved("9.9", &tasks));
    }

    #[test]
    fn blocker_resolved_phase_level() {
        let tasks = vec![
            Task {
                id: "1.1".into(),
                phase: 1,
                status: 'x',
                description: "Done".into(),
                blockers: vec![],
            },
            Task {
                id: "1.2".into(),
                phase: 1,
                status: 'x',
                description: "Also done".into(),
                blockers: vec![],
            },
            Task {
                id: "2.1".into(),
                phase: 2,
                status: ' ',
                description: "Pending".into(),
                blockers: vec![],
            },
        ];

        assert!(blocker_resolved("Phase 1", &tasks));
        assert!(!blocker_resolved("Phase 2", &tasks));
    }

    #[test]
    fn check_completion_all_done() {
        let f = write_tasks(
            "\
## Phase 1:

- [x] **1.1** Done
- [x] **1.2** Also done
",
        );

        let tasks = parse_tasks(f.path()).unwrap();
        assert!(check_completion(&tasks, None));
        assert!(check_completion(&tasks, Some(1)));
    }

    #[test]
    fn check_completion_not_done() {
        let f = write_tasks(
            "\
## Phase 1:

- [x] **1.1** Done
- [ ] **1.2** Pending
",
        );

        let tasks = parse_tasks(f.path()).unwrap();
        assert!(!check_completion(&tasks, None));
        assert!(!check_completion(&tasks, Some(1)));
    }

    #[test]
    fn check_completion_phase_filter() {
        let f = write_tasks(
            "\
## Phase 1:

- [x] **1.1** Done

## Phase 2:

- [ ] **2.1** Pending
",
        );

        let tasks = parse_tasks(f.path()).unwrap();
        assert!(!check_completion(&tasks, None));
        assert!(check_completion(&tasks, Some(1)));
        assert!(!check_completion(&tasks, Some(2)));
    }
}
