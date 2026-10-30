use crate::app::events;
use gpui_kit::*;
use serde_json::json;
use std::collections::HashSet;

use super::SettingsView;
impl SettingsView {
    pub(super) fn search_changed(&mut self, value: String, _: &mut Window, cx: &mut Context<Self>) {
        let query = value.trim().to_string();

        if query == self.query {
            return;
        }
        self.search_results.clear();
        if query.is_empty() && !self.selected_only {
            self.disclosures.end_filter();
        } else {
            self.disclosures.begin_filter();
        }
        self.query = query;
        self.send(
            json!({"action":"searchCatalog", "value":self.query, "enabled":!self.query.is_empty() && !self.selected_only}),
            cx,
        );
        // Revealing paths belongs to this explicit search edit. Snapshot
        // refreshes update data without changing the tree's expansion state.
        if !self.query.is_empty() {
            let catalog = if self.selected_only {
                crate::domain::catalog::selected(
                    &self.snapshot.workspaces,
                    &self.snapshot.preferences.allowed,
                )
            } else {
                self.snapshot.workspaces.clone()
            };
            self.disclosures
                .reveal_for_search(crate::domain::catalog::search_paths(
                    &crate::domain::catalog::filtered(&catalog, &self.query),
                ));
        }
    }
    pub(super) fn toggle_selected(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.search_results.clear();
        self.selected_only = !self.selected_only;
        if self.selected_only {
            self.disclosures
                .show_only(crate::domain::catalog::selected_paths(
                    &self.snapshot.preferences.allowed,
                ));
        } else if self.query.is_empty() {
            self.disclosures.end_filter();
        } else {
            self.disclosures
                .reveal_for_search(crate::domain::catalog::search_paths(
                    &self.filtered_catalog(),
                ));
        }
        self.send(
            json!({"action":"searchCatalog", "value":self.query, "enabled":!self.selected_only && !self.query.is_empty()}),
            cx,
        );
    }
    pub(super) fn load_selected_projects(&self) {
        let projects: HashSet<_> = self
            .snapshot
            .preferences
            .allowed
            .iter()
            .map(|port| (port.workspace_id.clone(), port.project_id.clone()))
            .collect();
        for (workspace, project) in projects {
            if !self.snapshot.loaded_projects.contains(&project)
                && !self.snapshot.loading.contains(&project)
                && !self.snapshot.node_errors.contains_key(&project)
                && self
                    .snapshot
                    .workspaces
                    .iter()
                    .any(|w| w.id == workspace && w.projects.iter().any(|p| p.id == project))
            {
                events::dispatch(
                    json!({"action":"loadProject", "workspaceID":workspace, "projectID":project}),
                );
            }
        }
    }
    pub(super) fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_results.clear();
        self.query.clear();
        self.selected_only = false;
        self.disclosures.end_filter();
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.send(json!({"action":"searchCatalog", "enabled":false}), cx);
    }
    pub(super) fn set_disclosure_open(
        &mut self,
        id: &str,
        open: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.query.is_empty() || self.selected_only {
            self.search_results.pin();
            if open {
                let latest = self.filtered_catalog();
                self.search_results
                    .open_branch(id, &latest, &self.snapshot.loaded_projects);
            } else {
                self.search_results.close_branch(id);
            }
        }

        let changed = self.disclosures.set_open(id, open);

        cx.notify();
        changed
    }
}
