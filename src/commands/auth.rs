use crate::auth;
use crate::error;
use crate::error::Result;
use colored::Colorize;

pub fn cmd_auth_login(provider_name: Option<String>, set_default: bool) -> Result<()> {
    use auth::providers::Provider;
    use dialoguer::{Password, Select};

    let provider = if let Some(name) = provider_name {
        Provider::from_str_loose(&name).ok_or_else(|| {
            error::MemoryError::Auth(format!(
                "Unknown provider: {}. Use: anthropic, openai, ollama",
                name
            ))
        })?
    } else {
        // Interactive selection
        let items: Vec<&str> = Provider::all().iter().map(|p| p.display_name()).collect();
        let selection = Select::new()
            .with_prompt("Select LLM provider")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| error::MemoryError::Auth(format!("Selection cancelled: {}", e)))?;
        Provider::all()[selection]
    };

    if !provider.requires_auth() {
        println!(
            "{} {} does not require authentication.",
            "Note:".cyan(),
            provider.display_name()
        );
        // Set as default if requested, or if no default is currently set
        let mut store = auth::AuthStore::load()?;
        if set_default || store.default_provider.is_none() {
            store.default_provider = Some(provider.to_string());
            store.save()?;
            println!(
                "{} Set {} as default provider.",
                "Done!".green().bold(),
                provider.display_name()
            );
        }
        return Ok(());
    }

    // Prompt for API key
    let key = Password::new()
        .with_prompt(format!("Enter {} API key", provider.display_name()))
        .interact()
        .map_err(|e| error::MemoryError::Auth(format!("Input cancelled: {}", e)))?;

    if key.trim().is_empty() {
        return Err(error::MemoryError::Auth("API key cannot be empty".into()));
    }

    let mut store = auth::AuthStore::load()?;
    store.set(
        provider,
        auth::ProviderCredential {
            cred_type: "api".to_string(),
            key,
            endpoint: None,
            model: None,
        },
    );

    if set_default {
        store.default_provider = Some(provider.to_string());
    } else if store.default_provider.is_none() {
        // Auto-set as default if no default exists
        store.default_provider = Some(provider.to_string());
    }

    store.save()?;

    println!(
        "{} Logged in to {}.",
        "Done!".green().bold(),
        provider.display_name()
    );
    if store.default_provider.as_deref() == Some(&provider.to_string()) {
        println!("  Set as default provider.");
    }

    Ok(())
}

pub fn cmd_auth_list() -> Result<()> {
    use auth::providers::Provider;

    let store = auth::AuthStore::load()?;

    println!("{}", "Configured Providers".green().bold());
    println!("{}", "=".repeat(50));

    for &provider in Provider::all() {
        let env_key = if !provider.env_var_name().is_empty() {
            std::env::var(provider.env_var_name()).ok()
        } else {
            None
        };

        let stored = store.get(provider);
        let is_default = store.default_provider.as_deref() == Some(&provider.to_string());

        let status = if env_key.is_some() {
            "env var".green().to_string()
        } else if stored.is_some() {
            "auth.json".cyan().to_string()
        } else if !provider.requires_auth() {
            "no auth needed".dimmed().to_string()
        } else {
            "not configured".dimmed().to_string()
        };

        let default_marker = if is_default { " (default)" } else { "" };

        println!(
            "  {}{}\t{}",
            provider.display_name().cyan().bold(),
            default_marker,
            status
        );
    }

    Ok(())
}

pub fn cmd_auth_logout(provider_name: &str) -> Result<()> {
    use auth::providers::Provider;

    let provider = Provider::from_str_loose(provider_name).ok_or_else(|| {
        error::MemoryError::Auth(format!(
            "Unknown provider: {}. Use: anthropic, openai, ollama",
            provider_name
        ))
    })?;

    let mut store = auth::AuthStore::load()?;
    store.remove(provider);
    store.save()?;

    println!(
        "{} Removed credentials for {}.",
        "Done!".green().bold(),
        provider.display_name()
    );

    Ok(())
}

pub fn cmd_auth_status() -> Result<()> {
    // Use the same resolution logic as Config
    let env_endpoint = std::env::var("ENGRAM_LLM_ENDPOINT").ok();
    let env_model = std::env::var("ENGRAM_LLM_MODEL").ok();

    match auth::resolve_provider(None, env_endpoint, env_model) {
        Ok(resolved) => {
            println!("{}", "Active LLM Provider".green().bold());
            println!("{}", "=".repeat(40));
            println!("  Provider:  {}", resolved.provider.display_name().cyan());
            println!("  Model:     {}", resolved.model);
            println!("  Endpoint:  {}", resolved.endpoint);

            if let Some(ref key) = resolved.api_key {
                let masked = if key.len() > 8 {
                    format!("{}...{}", &key[..4], &key[key.len() - 4..])
                } else {
                    "****".to_string()
                };
                println!("  API Key:   {}", masked);

                // Show source
                let env_var = resolved.provider.env_var_name();
                if !env_var.is_empty() && std::env::var(env_var).is_ok() {
                    println!("  Source:    {} (env var)", env_var);
                } else {
                    println!("  Source:    auth.json");
                }
            }
        }
        Err(e) => {
            // Distinguish config/IO errors from "genuinely no provider set"
            let msg = e.to_string();
            if msg.contains("No API key") || msg.contains("No provider") {
                println!(
                    "{} No provider configured. Using {} as fallback.",
                    "Note:".yellow(),
                    "Ollama (local)".cyan()
                );
                println!("  Run 'engram auth login' to configure a provider.");
            } else {
                println!("{} Failed to resolve provider: {}", "Error:".red(), e);
            }
        }
    }

    Ok(())
}
