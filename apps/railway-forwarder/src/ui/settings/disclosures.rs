//! Owns tree expansion separately from asynchronous catalog updates.
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct Disclosures {
    expanded: HashSet<String>,
    before_filter: Option<HashSet<String>>,
    manually_closed: HashSet<String>,
    manually_opened: HashSet<String>,
}

impl Disclosures {
    pub(super) fn new(expanded: HashSet<String>) -> Self {
        Self {
            expanded,
            ..Self::default()
        }
    }

    pub(super) fn contains(&self, id: &str) -> bool {
        self.expanded.contains(id)
    }

    pub(super) fn begin_filter(&mut self) {
        self.before_filter
            .get_or_insert_with(|| self.expanded.clone());
    }

    pub(super) fn end_filter(&mut self) {
        if let Some(expanded) = self.before_filter.take() {
            self.expanded = expanded;
        }
        self.apply_manual_choices();
    }

    pub(super) fn show_only(&mut self, paths: HashSet<String>) {
        self.begin_filter();
        // Clicking the selected-ports button explicitly requests these paths.
        self.manually_closed.retain(|path| !paths.contains(path));
        self.expanded = paths;
    }

    pub(super) fn reveal_for_search(&mut self, paths: HashSet<String>) {
        // Replace automatic paths from the previous query. Otherwise a broad
        // prefix such as "s" leaves unrelated rows pre-expanded for later data.
        self.expanded = paths;
        self.apply_manual_choices();
    }

    pub(super) fn set_open(&mut self, id: &str, open: bool) -> bool {
        let changed = if open {
            self.expanded.insert(id.into())
        } else {
            self.expanded.remove(id)
        };
        if !open {
            self.manually_opened.remove(id);
            self.manually_closed.insert(id.into());
        } else {
            self.manually_closed.remove(id);
            self.manually_opened.insert(id.into());
        }
        changed
    }

    fn apply_manual_choices(&mut self) {
        self.expanded.extend(self.manually_opened.iter().cloned());
        self.expanded
            .retain(|path| !self.manually_closed.contains(path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Environment, Project, Service, Workspace, catalog};

    #[test]
    fn closed_rows_survive_search_edits_and_clear_at_every_level() {
        let initial: HashSet<String> = ["w/w", "p/p", "e/p/e", "s/e/s"]
            .into_iter()
            .map(String::from)
            .collect();
        let mut tree = Disclosures::new(initial.clone());
        tree.begin_filter();
        tree.reveal_for_search(initial.clone());
        for id in ["s/e/s", "e/p/e", "p/p", "w/w"] {
            assert!(tree.set_open(id, false));
        }

        // A subsequent input event or search edit must not discard manual closes.
        tree.begin_filter();
        let mut later_results = initial.clone();
        later_results.insert("p/later-result".into());
        tree.reveal_for_search(later_results);
        for id in &initial {
            assert!(!tree.contains(id));
        }
        // Restoring the pre-search tree must not reopen those rows either.
        tree.end_filter();
        assert!(tree.expanded.is_empty());

        // Only an explicit click reopens a manually closed row.
        for id in ["w/w", "p/p", "e/p/e", "s/e/s"] {
            assert!(tree.set_open(id, true));
        }
        tree.begin_filter();
        tree.end_filter();
        assert_eq!(tree.expanded, initial);
    }

    #[test]
    fn selected_ports_are_an_explicit_reveal_and_clear_preserves_later_choices() {
        let initial = HashSet::from(["w/w".into(), "p/unfiltered".into()]);
        let selected = HashSet::from(["w/w".into(), "p/p".into(), "s/e/s".into()]);
        let mut tree = Disclosures::new(initial.clone());
        tree.show_only(selected.clone());
        assert!(tree.set_open("s/e/s", false));
        tree.begin_filter();
        tree.reveal_for_search(selected.clone());
        assert!(!tree.contains("s/e/s"));
        tree.show_only(selected);
        assert!(tree.contains("s/e/s"));
        assert!(tree.set_open("w/w", false));
        tree.end_filter();
        assert!(!tree.contains("w/w"));
        assert!(tree.contains("p/unfiltered"));
    }

    #[test]
    fn narrowing_a_query_removes_prefix_expansion_before_late_matches_arrive() {
        let mut workspaces = vec![Workspace {
            id: "w".into(),
            name: "Workspace".into(),
            projects: vec![
                Project {
                    id: "station".into(),
                    name: "Station".into(),
                    environments: vec![Environment {
                        id: "station-env".into(),
                        services: vec![Service {
                            id: "api-station".into(),
                            name: "api".into(),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                },
                Project {
                    id: "shared".into(),
                    name: "Shared Platform".into(),
                    environments: vec![Environment {
                        id: "shared-env".into(),
                        services: vec![Service {
                            id: "api-shared".into(),
                            name: "api".into(),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                },
            ],
        }];
        let mut tree = Disclosures::default();
        tree.begin_filter();
        tree.reveal_for_search(catalog::search_paths(&catalog::filtered(&workspaces, "s")));
        assert!(tree.contains("p/shared"));
        tree.begin_filter();
        tree.reveal_for_search(catalog::search_paths(&catalog::filtered(
            &workspaces,
            "station",
        )));
        assert!(tree.contains("p/station"));
        assert!(!tree.contains("p/shared"));

        workspaces[0].projects[1].environments.push(Environment {
            id: "e".into(),
            name: "production".into(),
            services: vec![Service {
                id: "s".into(),
                name: "station worker".into(),
                ..Default::default()
            }],
        });
        let later = catalog::filtered(&workspaces, "station");
        assert_eq!(later[0].projects.len(), 2);
        assert!(!tree.contains("p/shared"));
        assert!(!tree.contains("e/shared/e"));
    }

    #[test]
    fn repeated_close_activations_from_one_rendered_frame_are_idempotent() {
        let mut tree = Disclosures::new(HashSet::from(["p/p".into()]));
        assert!(tree.set_open("p/p", false));
        assert!(!tree.set_open("p/p", false));
        assert!(!tree.contains("p/p"));
        assert!(tree.set_open("p/p", true));
        assert!(tree.contains("p/p"));
    }
}
