use super::{main_thread, text};
use crate::app::events;
use anyhow::{Result, anyhow};
use block2::RcBlock;
use objc2::{
    AnyThread, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::ProtocolObject,
};
use objc2_app_kit::*;
use objc2_authentication_services::*;
use objc2_foundation::{NSError, NSObject, NSObjectProtocol, NSURL};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};
use tokio::sync::oneshot;
use uuid::Uuid;
thread_local! {static AUTH: RefCell<Option<AuthSession>> = const {RefCell::new(None)};}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind=MainThreadOnly]
    struct AuthAnchor;
    unsafe impl NSObjectProtocol for AuthAnchor {}
    unsafe impl ASWebAuthenticationPresentationContextProviding for AuthAnchor {
        #[unsafe(method_id(presentationAnchorForWebAuthenticationSession:))]
        unsafe fn presentation_anchor(
            &self,
            _session: &ASWebAuthenticationSession,
        ) -> Retained<ASPresentationAnchor> {
            let app = NSApplication::sharedApplication(self.mtm());
            let window = app
                .keyWindow()
                .or_else(|| app.windows().firstObject())
                .unwrap_or_else(|| unsafe { NSWindow::new(self.mtm()) });
            unsafe { Retained::cast_unchecked(window) }
        }
    }
);
struct AuthSession {
    id: Uuid,
    session: Retained<ASWebAuthenticationSession>,
    _anchor: Retained<AuthAnchor>,
}
pub fn authenticate(url: String, reply: oneshot::Sender<Result<String>>) {
    if reply.is_closed() {
        return;
    }
    unsafe {
        cancel_auth();
        let id = Uuid::new_v4();
        let reply = Arc::new(Mutex::new(Some(reply)));
        let completion_reply = reply.clone();
        let completion = RcBlock::new(move |url: *mut NSURL, error: *mut NSError| {
            let result = if let Some(url) = url.as_ref() {
                Ok(url.absoluteString().unwrap().to_string())
            } else if let Some(error) = error.as_ref() {
                if error.code() == 1 && &*error.domain() == ASWebAuthenticationSessionErrorDomain {
                    Err(anyhow!("Sign in was canceled."))
                } else {
                    Err(anyhow!("{}", error.localizedDescription()))
                }
            } else {
                Err(anyhow!("Sign in was canceled."))
            };
            if let Some(reply) = completion_reply.lock().unwrap().take() {
                let _ = reply.send(result);
            }
            events::emit(events::Event::AuthFinished(id));
        });
        let Some(url) = NSURL::URLWithString(&text(&url)) else {
            if let Some(reply) = reply.lock().unwrap().take() {
                let _ = reply.send(Err(anyhow!("Invalid Railway sign-in URL.")));
            }
            return;
        };
        let callback = ASWebAuthenticationSessionCallback::callbackWithCustomScheme(&text(
            "com.brody.railway-forwarder",
        ));
        let session = ASWebAuthenticationSession::initWithURL_callback_completionHandler(
            ASWebAuthenticationSession::alloc(),
            &url,
            &callback,
            &*completion as *const _ as *mut _,
        );
        let anchor: Retained<AuthAnchor> = msg_send![AuthAnchor::alloc(main_thread()), init];
        session.setPresentationContextProvider(Some(ProtocolObject::from_ref(&*anchor)));
        if !session.start() {
            if let Some(reply) = reply.lock().unwrap().take() {
                let _ = reply.send(Err(anyhow!("macOS could not open Railway sign in.")));
            }
            return;
        }
        AUTH.with(|slot| {
            *slot.borrow_mut() = Some(AuthSession {
                id,
                session,
                _anchor: anchor,
            })
        });
    }
}
pub fn finish_auth(id: Uuid) {
    AUTH.with(|slot| {
        if slot
            .borrow()
            .as_ref()
            .is_some_and(|session| session.id == id)
        {
            *slot.borrow_mut() = None;
        }
    });
}
pub fn cancel_auth() {
    AUTH.with(|slot| {
        if let Some(session) = slot.borrow_mut().take() {
            unsafe {
                session.session.cancel();
            }
        }
    });
}
