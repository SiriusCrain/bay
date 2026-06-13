pub const APP_BUNDLE_ID: &str = "org.siriuscrain.bay";
pub const APP_BUNDLE_NAME: &str = "Bay.app";

#[cfg(target_os = "macos")]
pub const APP_PROCESS_NAME: &str = "bay-desktop";
#[cfg(target_os = "linux")]
pub const APP_PROCESS_NAME: &str = "bay-desktop";

#[cfg(windows)]
pub const APP_PROCESS_NAME: &str = "bay-desktop.exe";

/// The name configured under `"package.productName"` in the tauri.conf.json file.
pub const TAURI_PRODUCT_NAME: &str = "bay-desktop";

pub const CLI_BINARY_NAME: &str = "bay";
pub const CLI_BINARY_NAME_MINIMAL: &str = "bay-minimal";
pub const PTY_BINARY_NAME: &str = "bayterm";

pub const CLI_CRATE_NAME: &str = "bay_cli";

pub const URL_SCHEMA: &str = "bay";

pub const PRODUCT_NAME: &str = "Bay";

pub const RUNTIME_DIR_NAME: &str = "bayrun";

/// Data directory name used in paths like ~/.local/share/{DATA_DIR_NAME}
#[cfg(unix)]
pub const DATA_DIR_NAME: &str = "bay";
#[cfg(windows)]
pub const DATA_DIR_NAME: &str = "Bay";

/// Backup directory name
pub const BACKUP_DIR_NAME: &str = ".bay.dotfiles.bak";

pub const GITHUB_REPO_NAME: &str = "SiriusCrain/bay";

/// Build time env vars
pub mod build {
    /// The target of the current build, e.g. "aarch64-unknown-linux-musl"
    pub const TARGET_TRIPLE: Option<&str> = option_env!("BAY_BUILD_TARGET_TRIPLE");

    /// The variant of the current build
    pub const VARIANT: Option<&str> = option_env!("BAY_BUILD_VARIANT");

    /// A git full sha hash of the current build
    pub const HASH: Option<&str> = option_env!("BAY_BUILD_HASH");

    /// The datetime in rfc3339 format of the current build
    pub const DATETIME: Option<&str> = option_env!("BAY_BUILD_DATETIME");

    /// If `fish` tests should be skipped
    pub const SKIP_FISH_TESTS: bool = option_env!("BAY_BUILD_SKIP_FISH_TESTS").is_some();

    /// If `shellcheck` tests should be skipped
    pub const SKIP_SHELLCHECK_TESTS: bool = option_env!("BAY_BUILD_SKIP_SHELLCHECK_TESTS").is_some();
}

/// macOS specific constants
pub mod macos {
    pub const BUNDLE_CONTENTS_MACOS_PATH: &str = "Contents/MacOS";
    pub const BUNDLE_CONTENTS_RESOURCE_PATH: &str = "Contents/Resources";
    pub const BUNDLE_CONTENTS_HELPERS_PATH: &str = "Contents/Helpers";
    pub const BUNDLE_CONTENTS_INFO_PLIST_PATH: &str = "Contents/Info.plist";
}

pub mod linux {
    pub const DESKTOP_ENTRY_NAME: &str = "bay-desktop.desktop";

    /// Name of the deb package.
    pub const PACKAGE_NAME: &str = "bay";

    /// The wm_class used for the application windows.
    pub const DESKTOP_APP_WM_CLASS: &str = "Bay";
}

pub mod env_var {
    macro_rules! define_env_vars {
        ($($(#[$meta:meta])* $ident:ident = $name:expr),*) => {
            $(
                $(#[$meta])*
                pub const $ident: &str = $name;
            )*

            pub const ALL: &[&str] = &[$($ident),*];
        }
    }

    define_env_vars! {
        /// The UUID of the current parent bayterm instance
        BAYTERM_SESSION_ID = "BAYTERM_SESSION_ID",

        /// The current parent socket to connect to
        BAY_PARENT = "BAY_PARENT",

        /// Set the [`BAY_PARENT`] parent socket to connect to
        BAY_SET_PARENT = "BAY_SET_PARENT",

        /// Guard for the [`BAY_SET_PARENT`] check
        BAY_SET_PARENT_CHECK = "BAY_SET_PARENT_CHECK",

        /// Set if bayterm is running, contains the version
        BAY_TERM = "BAY_TERM",

        /// Sets the current log level
        BAY_LOG_LEVEL = "BAY_LOG_LEVEL",

        /// Overrides the ZDOTDIR environment variable
        BAY_ZDOTDIR = "BAY_ZDOTDIR",

        /// Indicates a process was launched by Bay
        PROCESS_LAUNCHED_BY_BAY = "PROCESS_LAUNCHED_BY_BAY",

        /// The shell to use in bayterm
        BAY_SHELL = "BAY_SHELL",

        /// Indicates the user is debugging the shell
        BAY_DEBUG_SHELL = "BAY_DEBUG_SHELL",

        /// Overrides the path to the bundle metadata released with certain desktop builds.
        BAY_BUNDLE_METADATA_PATH = "BAY_BUNDLE_METADATA_PATH"
    }
}

pub mod system_paths {
    /// System installation paths
    pub const APPLICATIONS_DIR: &str = "/Applications";
    pub const USR_LOCAL_BIN: &str = "/usr/local/bin";
    pub const USR_SHARE: &str = "/usr/share";
    pub const OPT_HOMEBREW_BIN: &str = "/opt/homebrew/bin";
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    use super::*;

    #[test]
    fn test_build_envs() {
        if let Some(build_variant) = build::VARIANT {
            println!("build_variant: {build_variant}");
            assert!(["full", "minimal"].contains(&&*build_variant.to_ascii_lowercase()));
        }

        if let Some(build_hash) = build::HASH {
            println!("build_hash: {build_hash}");
            assert!(!build_hash.is_empty());
        }

        if let Some(build_datetime) = build::DATETIME {
            println!("build_datetime: {build_datetime}");
            println!("{}", OffsetDateTime::parse(build_datetime, &Rfc3339).unwrap());
        }
    }
}
