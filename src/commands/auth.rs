use crate::auth::{self, AuthStore, ProviderCredential};
use crate::error::{self, MemoryError, Result};
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

pub fn cmd_auth_test(provider_name: Option<String>) -> Result<()> {
    use crate::commands::provider_test::{test_all_providers_sync, test_provider_sync};
    use auth::providers::Provider;

    let results = match provider_name {
        Some(name) => {
            let p = Provider::from_str_loose(&name)
                .ok_or_else(|| MemoryError::Auth(format!("Unknown provider: {}", name)))?;
            vec![test_provider_sync(p)]
        }
        None => test_all_providers_sync(),
    };

    println!("{}", "Provider Connectivity Test".green().bold());
    println!("{}", "=".repeat(60));
    for r in &results {
        if r.success {
            println!(
                "  {} {:<24} {}ms  model: {}",
                "✓".green().bold(),
                r.provider.display_name().cyan(),
                r.latency_ms,
                r.model
            );
            if !r.response_snippet.is_empty() {
                println!("    \"{}\"", r.response_snippet.dimmed());
            }
        } else {
            println!(
                "  {} {:<24} {}",
                "✗".red().bold(),
                r.provider.display_name().cyan(),
                r.error.as_deref().unwrap_or("unknown error").red()
            );
        }
    }
    Ok(())
}

pub fn cmd_auth_model(provider_name: &str, model: &str) -> Result<()> {
    use auth::providers::Provider;

    let provider = Provider::from_str_loose(provider_name)
        .ok_or_else(|| MemoryError::Auth(format!("Unknown provider: {}", provider_name)))?;
    let mut store = AuthStore::load()?;
    if let Some(cred) = store.providers.get_mut(&provider.to_string()) {
        cred.model = Some(model.to_string());
    } else {
        // No credential yet — store model only (key empty, validated at inference time)
        store.set(
            provider,
            ProviderCredential {
                cred_type: "api".to_string(),
                key: String::new(),
                endpoint: None,
                model: Some(model.to_string()),
            },
        );
    }
    store.save()?;
    println!(
        "{} Set model for {} to {}",
        "Done!".green().bold(),
        provider.display_name().cyan(),
        model.yellow()
    );
    Ok(())
}

pub fn cmd_auth_models(provider_name: Option<String>, embed: bool) -> Result<()> {
    use crate::commands::provider_test::{fetch_embed_models_sync, fetch_models_sync};
    use auth::providers::Provider;

    // --embed: list embedding models for the given provider name (string, not Provider enum)
    if embed {
        let embed_pname: String = match provider_name {
            Some(ref n) => n.clone(),
            None => AuthStore::load()
                .ok()
                .and_then(|s| s.embed_provider)
                .unwrap_or_else(|| "ollama".to_string()),
        };

        println!(
            "{} Fetching embedding models for {}...",
            "→".cyan(),
            embed_pname.cyan()
        );

        let models = fetch_embed_models_sync(&embed_pname)?;
        if models.is_empty() {
            println!("{} No embedding models found.", "Note:".yellow());
            return Ok(());
        }
        println!(
            "{} {} embedding models:\n",
            "Found".green().bold(),
            models.len()
        );
        for m in &models {
            println!("  • {}", m);
        }
        println!(
            "\n{} engram auth embed-model {} <model-name>",
            "To use one:".dimmed(),
            embed_pname
        );
        return Ok(());
    }

    // LLM models
    let provider = match provider_name {
        Some(ref name) => Provider::from_str_loose(name)
            .ok_or_else(|| MemoryError::Auth(format!("Unknown provider: {}", name)))?,
        None => {
            let env_endpoint = std::env::var("ENGRAM_LLM_ENDPOINT").ok();
            let env_model = std::env::var("ENGRAM_LLM_MODEL").ok();
            let resolved = auth::resolve_provider(None, env_endpoint, env_model)?;
            resolved.provider
        }
    };

    if !provider.supports_model_list() {
        return Err(MemoryError::Auth(format!(
            "{} does not expose a /models endpoint. Try: openai, ollama, vscode, openrouter",
            provider.display_name()
        )));
    }

    println!(
        "{} Fetching LLM models from {}...",
        "→".cyan(),
        provider.display_name().cyan()
    );

    let models = fetch_models_sync(provider)?;
    if models.is_empty() {
        println!("{} No models returned.", "Note:".yellow());
        return Ok(());
    }

    println!(
        "{} {} models available:\n",
        "Found".green().bold(),
        models.len()
    );
    for model in &models {
        if model.ends_with(":free") {
            println!("  {} {}", "★".green(), model.green());
        } else {
            println!("  • {}", model);
        }
    }
    println!(
        "\n{} engram auth model {} <model-name>",
        "To use one:".dimmed(),
        provider
    );

    Ok(())
}

pub fn cmd_auth_embed(provider_name: &str) -> Result<()> {
    let mut store = AuthStore::load()?;
    store.embed_provider = Some(provider_name.to_string());
    store.save()?;
    println!(
        "{} Set embedding provider to {}",
        "Done!".green().bold(),
        provider_name.cyan()
    );
    Ok(())
}

pub fn cmd_auth_embed_model(provider_name: &str, model: &str) -> Result<()> {
    let mut store = AuthStore::load()?;
    // Also set the provider if not already set
    if store.embed_provider.is_none() {
        store.embed_provider = Some(provider_name.to_string());
    }
    store.embed_model = Some(model.to_string());
    store.save()?;
    println!(
        "{} Set embedding model to {} (provider: {})",
        "Done!".green().bold(),
        model.yellow(),
        provider_name.cyan()
    );
    Ok(())
}

pub fn cmd_auth_status() -> Result<()> {
    use auth::providers::Provider;

    let env_endpoint = std::env::var("ENGRAM_LLM_ENDPOINT").ok();
    let env_model = std::env::var("ENGRAM_LLM_MODEL").ok();
    let store = AuthStore::load().unwrap_or_default();

    // --- Active provider ---
    println!("{}", "Active Provider".green().bold());
    println!("{}", "─".repeat(55));
    match auth::resolve_provider(None, env_endpoint, env_model) {
        Ok(resolved) => {
            println!(
                "  Provider : {}",
                resolved.provider.display_name().cyan().bold()
            );
            println!("  Model    : {}", resolved.model.yellow());
            println!("  Endpoint : {}", resolved.endpoint);
            if let Some(ref key) = resolved.api_key {
                let masked = if key.len() > 8 {
                    format!("{}...{}", &key[..4], &key[key.len() - 4..])
                } else {
                    "****".to_string()
                };
                let env_var = resolved.provider.env_var_name();
                let source = if !env_var.is_empty() && std::env::var(env_var).is_ok() {
                    format!("{} (env var)", env_var)
                } else {
                    "auth.json".to_string()
                };
                println!("  API Key  : {}  ({})", masked, source.dimmed());
            }
        }
        Err(_) => {
            println!(
                "  {} No provider configured — using Ollama fallback.",
                "Note:".yellow()
            );
        }
    }

    // --- Embedding provider ---
    let embed = store
        .embed_provider
        .as_deref()
        .unwrap_or("inferred from LLM provider");
    let embed_model_display = store
        .embed_model
        .as_deref()
        .unwrap_or("default for provider");
    println!("\n  Embed provider : {}", embed.cyan());
    println!("  Embed model    : {}", embed_model_display.yellow());

    // --- All providers overview ---
    println!("\n{}", "All Providers".green().bold());
    println!("{}", "─".repeat(55));
    println!(
        "  {:<22} {:<14} {:<12} Current model",
        "Provider", "Auth", "Models API"
    );
    println!("  {}", "─".repeat(52));

    for &p in Provider::all() {
        let env_key = if !p.env_var_name().is_empty() {
            std::env::var(p.env_var_name()).ok()
        } else {
            None
        };
        let stored = store.get(p);
        let is_default = store.default_provider.as_deref() == Some(&p.to_string());

        let auth_status = if env_key.is_some() {
            "env var".green().to_string()
        } else if stored.map(|c| !c.key.is_empty()).unwrap_or(false) {
            "auth.json".cyan().to_string()
        } else if !p.requires_auth() {
            "not needed".dimmed().to_string()
        } else {
            "not set".dimmed().to_string()
        };

        let models_api = if p.supports_model_list() {
            "yes".green().to_string()
        } else {
            "─".dimmed().to_string()
        };

        let model = stored
            .and_then(|c| c.model.as_deref())
            .unwrap_or(p.default_model());

        let default_tag = if is_default { " ◀ default" } else { "" };

        println!(
            "  {:<22} {:<22} {:<20} {}{}",
            p.display_name().cyan().to_string(),
            auth_status,
            models_api,
            model.dimmed(),
            default_tag.yellow()
        );
    }

    println!("\n{}", "Quick setup — LLM".green().bold());
    println!(
        "  engram auth login --provider openrouter   # API key from openrouter.ai (100+ models)"
    );
    println!("  engram auth login --provider vscode       # no key needed (VS Code LM bridge)");
    println!("  engram auth login --provider ollama       # no key needed (local)");
    println!("  engram auth models openrouter             # list LLM models incl. free ★");
    println!("  engram auth models ollama                 # list locally installed models");
    println!("  engram auth model <provider> <model>      # set LLM model for provider");
    println!("  engram auth test                          # ping all configured providers");
    println!("\n{}", "Quick setup — Embeddings".green().bold());
    println!("  engram auth models --embed openai         # list OpenAI embedding models");
    println!("  engram auth models --embed ollama         # list Ollama embedding models");
    println!("  engram auth models --embed gemini         # list Gemini embedding models");
    println!("  engram auth embed-model ollama nomic-embed-text   # set embedding model");

    Ok(())
}
