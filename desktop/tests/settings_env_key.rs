//! Applying summary preferences preserves environment-only keys in memory.

use hn_desktop::settings::Settings;
use hn_lib::reader::{self, Provider};

#[test]
fn apply_saves_only_edited_keys_for_both_providers() {
    const CHILD_ROOT: &str = "HN_TEST_SETTINGS_ENV_KEY_ROOT";
    const OPENROUTER_ENV: &str = "sk-or-env-dummy";
    const ANTHROPIC_ENV: &str = "sk-ant-env-dummy";
    const OPENROUTER_EDIT: &str = "sk-or-edited-dummy";
    const ANTHROPIC_EDIT: &str = "sk-ant-edited-dummy";

    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        let root = std::env::temp_dir().join(format!("hn-settings-env-key-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        // Give the child fake keys without mutating the test process's environment.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "apply_saves_only_edited_keys_for_both_providers"])
            .env(CHILD_ROOT, &root)
            .env("OPENROUTER_API_KEY", OPENROUTER_ENV)
            .env("ANTHROPIC_API_KEY", ANTHROPIC_ENV)
            .output();
        // A stale `--exact` name would run no test and still succeed; the file proves the child ran.
        let wrote = root.join("summary.json").exists();
        std::fs::remove_dir_all(&root).unwrap();
        let output = output.unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
        assert!(wrote, "the child test did not run");
        return;
    };

    let path = std::path::PathBuf::from(root).join("summary.json");
    for (provider, environment, edited) in [
        (Provider::OpenRouter, OPENROUTER_ENV, OPENROUTER_EDIT),
        (Provider::Anthropic, ANTHROPIC_ENV, ANTHROPIC_EDIT),
    ] {
        let before = reader::load_config(&path).unwrap();
        assert_eq!(before.key_for(provider), environment);
        let mut settings = Settings::open(&before, Some(path.clone()), false);
        settings.set_provider(provider);
        settings.toggle_automatic();
        let applied = settings.apply().unwrap();
        assert_eq!(applied.key_for(provider), environment);
        let reloaded = reader::load_config(&path).unwrap();
        let saved = match provider {
            Provider::OpenRouter => &reloaded.api_key,
            Provider::Anthropic => &reloaded.anthropic_api_key,
        };
        assert!(saved.is_empty(), "{provider:?} environment key was written to summary.json");
        assert_eq!(reloaded.provider, provider);
        assert_eq!(reloaded.automatic, !before.automatic);

        settings.set_key(edited);
        let applied = settings.apply().unwrap();
        let fallback = match provider {
            Provider::OpenRouter => &applied.environment_key,
            Provider::Anthropic => &applied.anthropic_environment_key,
        };
        assert!(fallback.is_empty());
        assert_eq!(applied.key_for(provider), edited);
    }

    let reloaded = reader::load_config(&path).unwrap();
    assert_eq!(reloaded.api_key, OPENROUTER_EDIT);
    assert_eq!(reloaded.anthropic_api_key, ANTHROPIC_EDIT);
    assert_eq!(reloaded.key_for(Provider::OpenRouter), OPENROUTER_EDIT);
    assert_eq!(reloaded.key_for(Provider::Anthropic), ANTHROPIC_EDIT);
}
