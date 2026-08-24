//! Problems with authored content, keyed by the file they are in — per-file and
//! replaced on recompile, unlike [`crate::runtime_warnings`]' time-ordered log.

use std::collections::BTreeMap;

use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProblemSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct ContentProblem {
    pub severity: ProblemSeverity,
    /// What went wrong, in the compiler's own words.
    pub message: String,
    /// Line in the *generated* shader, where the producer can place one. A
    /// graph has no lines of its own, so this is usually `None`.
    pub line: Option<usize>,
}

/// Problems by source path, project-relative.
///
/// `BTreeMap` so the panel's row order is the same every frame — a `HashMap`
/// reshuffles the list on each rebuild.
#[derive(Resource, Default, Debug)]
pub struct ContentProblems {
    by_path: BTreeMap<String, Vec<ContentProblem>>,
}

impl ContentProblems {
    /// Replace everything known about `path`. An empty `problems` clears it, so
    /// a producer can call this unconditionally after every compile.
    pub fn set(&mut self, path: impl Into<String>, problems: Vec<ContentProblem>) {
        let path = path.into();
        if problems.is_empty() {
            self.by_path.remove(&path);
        } else {
            self.by_path.insert(path, problems);
        }
    }

    pub fn clear_path(&mut self, path: &str) {
        self.by_path.remove(path);
    }

    pub fn get(&self, path: &str) -> &[ContentProblem] {
        self.by_path.get(path).map_or(&[], Vec::as_slice)
    }

    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    /// `(path, problem)` pairs in path order, errors before warnings within a
    /// path.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ContentProblem)> {
        self.by_path
            .iter()
            .flat_map(|(path, list)| list.iter().map(move |p| (path.as_str(), p)))
    }

    pub fn error_count(&self) -> usize {
        self.iter()
            .filter(|(_, p)| p.severity == ProblemSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.iter()
            .filter(|(_, p)| p.severity == ProblemSeverity::Warning)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(message: &str) -> ContentProblem {
        ContentProblem {
            severity: ProblemSeverity::Error,
            message: message.to_string(),
            line: None,
        }
    }

    /// The panel must show a repaired file as fixed. `set` replaces rather than
    /// appends, so recompiling clean is what clears it.
    #[test]
    fn recompiling_clean_clears_the_path() {
        let mut problems = ContentProblems::default();
        problems.set("materials/a.material", vec![err("boom")]);
        assert_eq!(problems.error_count(), 1);

        problems.set("materials/a.material", Vec::new());
        assert!(problems.is_empty());
    }

    #[test]
    fn one_path_does_not_clear_another() {
        let mut problems = ContentProblems::default();
        problems.set("a.material", vec![err("a broke")]);
        problems.set("b.material", vec![err("b broke")]);
        problems.set("a.material", Vec::new());

        assert_eq!(problems.get("a.material").len(), 0);
        assert_eq!(problems.get("b.material").len(), 1);
    }
}
