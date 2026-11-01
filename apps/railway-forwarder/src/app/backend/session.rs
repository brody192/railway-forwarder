use super::Backend;
use std::sync::Arc;
impl Backend {
    pub(super) async fn refresh_account(self: &Arc<Self>, generation: u64) {
        if !self.state.lock().unwrap().data.signed_in {
            return;
        }
        let Some(_loading) = self.loading("account", generation) else {
            return;
        };
        let result = self.api.oauth.profile().await;
        let picture = result
            .as_ref()
            .ok()
            .and_then(|profile| profile.picture.clone());
        self.mutate(|state| {
            if state.generation == generation {
                match result {
                    Ok(profile) => {
                        state.data.email = Some(profile.email);
                        state.data.account_name = profile.name;
                        state.data.account_photo = None;
                        state.data.node_errors.remove("account");
                    }
                    Err(error) => {
                        state.data.node_errors.insert(
                            "account".into(),
                            format!("Could not load your Railway account. {error}"),
                        );
                    }
                }
            }
        });
        if let Some(picture) = picture {
            if self.state.lock().unwrap().generation != generation {
                return;
            }
            let photo = self
                .api
                .oauth
                .profile_photo(&picture)
                .await
                .ok()
                .map(Arc::new);
            self.mutate(|state| {
                if state.generation == generation && state.data.signed_in {
                    state.data.account_photo = photo;
                }
            });
        }
    }

    pub(super) fn sign_in(self: &Arc<Self>) {
        let mut state = self.state.lock().unwrap();
        if state.data.busy || state.data.demo {
            return;
        }
        state.generation += 1;
        state.loads.clear();
        let generation = state.generation;
        state.data.busy = true;
        state.data.error = None;
        let client = state.data.preferences.client_id.clone();
        drop(state);
        self.cancel_search();
        crate::app::events::changed();
        let backend = self.clone();
        *self.auth.lock().unwrap() = Some(self.runtime.spawn(async move {
            let result = backend.api.oauth.sign_in(&client).await;
            let success = result.is_ok();
            backend.mutate(|state| {
                if state.generation == generation {
                    state.data.busy = false;
                    match result {
                        Ok(()) => {
                            state.data.signed_in = true;
                            state.data.email = None;
                            state.data.account_name = None;
                            state.data.account_photo = None;
                            state.data.node_errors.remove("account");
                        }
                        Err(error) if error.to_string() == "Sign in was canceled." => {}
                        Err(error) => state.data.error = Some(format!("{error:#}")),
                    }
                }
            });
            if success && backend.state.lock().unwrap().generation == generation {
                tokio::join!(
                    backend.clone().refresh(generation),
                    backend.refresh_account(generation)
                );
            }
        }));
    }
    pub(super) fn sign_out(self: &Arc<Self>) {
        if self.state.lock().unwrap().data.demo {
            return;
        }
        self.mutate(|state| {
            state.generation += 1;
            state.loads.clear();
            state.data.busy = true;
        });
        if let Some(task) = self.auth.lock().unwrap().take() {
            task.abort();
        }
        self.cancel_search();
        self.stop_all();
        crate::app::events::cancel_authentication();
        let generation = self.state.lock().unwrap().generation;
        let backend = self.clone();
        self.runtime.spawn(async move {
            let result = backend.api.oauth.sign_out().await;
            backend.mutate(|state| {
                if state.generation == generation {
                    state.data.busy = false;
                    match result {
                        Ok(()) => {
                            state.data.signed_in = false;
                            state.data.email = None;
                            state.data.account_name = None;
                            state.data.account_photo = None;
                            state.data.workspaces.clear();
                            state.data.ports.clear();
                            state.data.scanned.clear();
                            state.data.loaded_projects.clear();
                            state.data.node_errors.clear();
                            state.loads.clear();
                        }
                        Err(error) => state.data.error = Some(format!("{error:#}")),
                    }
                }
            });
            if let Err(error) = backend.save() {
                backend.mutate(|state| state.data.error = Some(error.to_string()));
            }
        });
    }
}
