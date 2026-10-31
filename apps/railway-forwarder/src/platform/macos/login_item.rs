//! Native main-app login registration, with macOS as the source of truth.
use super::main_thread;
use crate::domain::LoginItemStatus;
use anyhow::{Result, bail};
use objc2_foundation::NSBundle;
use objc2_service_management::{SMAppService, SMAppServiceStatus};

fn from_native(status: SMAppServiceStatus) -> LoginItemStatus {
    match status {
        SMAppServiceStatus::NotRegistered => LoginItemStatus::NotRegistered,
        SMAppServiceStatus::Enabled => LoginItemStatus::Enabled,
        SMAppServiceStatus::RequiresApproval => LoginItemStatus::RequiresApproval,
        SMAppServiceStatus::NotFound => LoginItemStatus::NotFound,
        _ => LoginItemStatus::Unavailable,
    }
}

pub fn status() -> LoginItemStatus {
    let _main_thread = main_thread();
    let bundle = NSBundle::mainBundle();
    if bundle
        .bundleIdentifier()
        .map(|id| id.to_string())
        .as_deref()
        != Some("com.brody.railway-forwarder")
        || !bundle.bundlePath().to_string().ends_with(".app")
    {
        return LoginItemStatus::Unavailable;
    }
    // The application bundle is signed by the packaging script. No helper
    // application, LaunchAgent or shell command is needed for the main app.
    unsafe { from_native(SMAppService::mainAppService().status()) }
}

pub fn diagnostic_status() -> String {
    let _main_thread = main_thread();
    let bundle = NSBundle::mainBundle();
    format!(
        "Start at login: {:?}\nBundle: {}\nIdentifier: {}\nNative status: {:?}",
        status(),
        bundle.bundlePath(),
        bundle
            .bundleIdentifier()
            .map(|id| id.to_string())
            .unwrap_or_default(),
        unsafe { SMAppService::mainAppService().status() }
    )
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let before = status();
    if before == LoginItemStatus::Unavailable {
        bail!("Run the signed Railway Forwarder.app to configure start at login.");
    }
    if before.is_registered() == enabled {
        return Ok(());
    }
    unsafe {
        let service = SMAppService::mainAppService();
        let result = if enabled {
            service.registerAndReturnError()
        } else {
            service.unregisterAndReturnError()
        };
        let after = from_native(service.status());
        // macOS can return an approval-related error after registering the item.
        // Reflect its actual state and let the UI explain the required approval.
        if (enabled && after.is_registered())
            || (!enabled && after == LoginItemStatus::NotRegistered)
        {
            return Ok(());
        }
        if let Err(error) = result {
            bail!(
                "Could not change start at login. {}",
                error.localizedDescription()
            );
        }
    }
    bail!("macOS did not apply the start at login change. Try again.")
}

pub fn open_settings() {
    let _main_thread = main_thread();
    unsafe { SMAppService::openSystemSettingsLoginItems() };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_status_keeps_approval_distinct_from_enabled_and_handles_unknown_values() {
        for (native, expected, registered) in [
            (
                SMAppServiceStatus::NotRegistered,
                LoginItemStatus::NotRegistered,
                false,
            ),
            (SMAppServiceStatus::Enabled, LoginItemStatus::Enabled, true),
            (
                SMAppServiceStatus::RequiresApproval,
                LoginItemStatus::RequiresApproval,
                true,
            ),
            (
                SMAppServiceStatus::NotFound,
                LoginItemStatus::NotFound,
                false,
            ),
            (SMAppServiceStatus(99), LoginItemStatus::Unavailable, false),
        ] {
            let status = from_native(native);
            assert_eq!(status, expected);
            assert_eq!(status.is_registered(), registered);
        }
    }
}
