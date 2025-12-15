//! Internationalization support using Fluent

use fluent::{FluentArgs, FluentBundle, FluentResource};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use unic_langid::LanguageIdentifier;

/// Localization manager
pub struct I18n {
    bundles: RwLock<HashMap<String, Arc<FluentBundle<FluentResource>>>>,
    current_locale: RwLock<String>,
    fallback_locale: String,
}

impl I18n {
    /// Create a new I18n manager with default locale
    pub fn new(default_locale: &str) -> Self {
        Self {
            bundles: RwLock::new(HashMap::new()),
            current_locale: RwLock::new(default_locale.to_string()),
            fallback_locale: "en".to_string(),
        }
    }

    /// Load translations from a directory
    pub fn load_from_dir(&self, dir: &std::path::Path) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let locale = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("en");

                self.load_locale(locale, &path)?;
            }
        }

        Ok(())
    }

    /// Load a specific locale
    fn load_locale(&self, locale: &str, dir: &std::path::Path) -> anyhow::Result<()> {
        let lang_id: LanguageIdentifier = locale.parse()
            .map_err(|e| anyhow::anyhow!("Invalid locale {}: {}", locale, e))?;

        let mut bundle = FluentBundle::new(vec![lang_id]);

        // Load all .ftl files in the directory
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "ftl") {
                let content = std::fs::read_to_string(&path)?;
                let resource = FluentResource::try_new(content)
                    .map_err(|(_, errors)| {
                        anyhow::anyhow!("Fluent parse errors in {:?}: {:?}", path, errors)
                    })?;

                bundle.add_resource(resource)
                    .map_err(|errors| {
                        anyhow::anyhow!("Fluent bundle errors: {:?}", errors)
                    })?;
            }
        }

        self.bundles.write().insert(locale.to_string(), Arc::new(bundle));
        tracing::info!("Loaded locale: {}", locale);

        Ok(())
    }

    /// Set the current locale
    pub fn set_locale(&self, locale: &str) -> bool {
        if self.bundles.read().contains_key(locale) {
            *self.current_locale.write() = locale.to_string();
            true
        } else {
            false
        }
    }

    /// Get the current locale
    pub fn current_locale(&self) -> String {
        self.current_locale.read().clone()
    }

    /// Get a localized string
    pub fn get(&self, key: &str) -> String {
        self.get_with_args(key, None)
    }

    /// Get a localized string with arguments
    pub fn get_with_args(&self, key: &str, args: Option<&FluentArgs>) -> String {
        let bundles = self.bundles.read();
        let current = self.current_locale.read().clone();

        // Try current locale
        if let Some(bundle) = bundles.get(&current) {
            if let Some(msg) = bundle.get_message(key) {
                if let Some(pattern) = msg.value() {
                    let mut errors = Vec::new();
                    let result = bundle.format_pattern(pattern, args, &mut errors);

                    if errors.is_empty() {
                        return result.to_string();
                    }
                }
            }
        }

        // Try fallback locale
        if current != self.fallback_locale {
            if let Some(bundle) = bundles.get(&self.fallback_locale) {
                if let Some(msg) = bundle.get_message(key) {
                    if let Some(pattern) = msg.value() {
                        let mut errors = Vec::new();
                        let result = bundle.format_pattern(pattern, args, &mut errors);

                        if errors.is_empty() {
                            return result.to_string();
                        }
                    }
                }
            }
        }

        // Return key as fallback
        key.to_string()
    }

    /// Get available locales
    pub fn available_locales(&self) -> Vec<String> {
        self.bundles.read().keys().cloned().collect()
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new("ja")
    }
}

/// Convenience macro for getting localized strings
#[macro_export]
macro_rules! t {
    ($i18n:expr, $key:expr) => {
        $i18n.get($key)
    };
    ($i18n:expr, $key:expr, $($arg_name:ident = $arg_value:expr),+ $(,)?) => {{
        let mut args = fluent::FluentArgs::new();
        $(
            args.set(stringify!($arg_name), $arg_value);
        )+
        $i18n.get_with_args($key, Some(&args))
    }};
}
