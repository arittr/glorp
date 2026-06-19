use crate::{
    error::Result,
    paths::AppPaths,
    storage::{state::StateStore, usage_store::UsageStore},
    usage::{agentsview::AgentsviewCommandProvider, provider::UsageProvider},
};
use time::OffsetDateTime;

pub fn run() -> Result<()> {
    let paths = AppPaths::resolve()?;
    paths.ensure()?;
    println!("config_dir: {}", paths.config_dir.display());
    println!("state_json: {}", paths.state_file.display());
    println!("usage_sqlite: {}", paths.usage_db.display());

    match StateStore::new(paths.state_file.clone()).load() {
        Ok(Some(_)) => println!("state: readable"),
        Ok(None) => println!("state: not initialized"),
        Err(err) => println!("state: {err}"),
    }

    let mut usage_store = UsageStore::open(&paths.usage_db)?;
    let provider = AgentsviewCommandProvider::from_environment();
    let version = provider.discovered_version();
    let result = provider.poll(&mut usage_store)?;
    println!("provider: agentsview");
    if result.diagnostics.is_empty() {
        println!("helpers: found");
        println!("provider command health: ok");
        println!("Tokenmaxxing-compatible: yes");
    } else {
        println!("helpers: not found or blocked");
        println!("Tokenmaxxing-compatible: no");
        for diagnostic in &result.diagnostics {
            println!(
                "{}: {} - {}",
                diagnostic.provider_surface, diagnostic.code, diagnostic.message
            );
        }
        if result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_helper")
        {
            println!("Canonical provider blocked.");
            println!("Install agentsview or set GLORP_AGENTSVIEW_BIN to its executable path.");
        }
    }
    if let Some(version) = version {
        println!("helper version: agentsview provider={version} parser={version}");
    }

    let provider_versions = usage_store.provider_versions()?;
    if provider_versions.iter().any(is_legacy_provider_version) {
        println!("legacy provider data: present");
    }
    for helper in provider_versions {
        if is_legacy_provider_version(&helper) {
            println!(
                "legacy helper version: {} provider={} parser={}",
                helper.provider_surface, helper.provider_version, helper.parser_version
            );
        } else {
            println!(
                "helper version: {} provider={} parser={}",
                helper.provider_surface, helper.provider_version, helper.parser_version
            );
        }
    }

    let now = OffsetDateTime::now_utc();
    let recent_sources = usage_store
        .token_totals_by_source_between(now - time::Duration::hours(24), now)
        .unwrap_or_default();
    for (source, total) in recent_sources {
        println!("source: {} recent_24h={:.0}", source, total.max(0.0));
    }

    for diagnostic in usage_store.recent_diagnostics(5)? {
        println!(
            "recent diagnostic: {} {} {}",
            diagnostic.provider_surface, diagnostic.code, diagnostic.message
        );
    }
    Ok(())
}

fn is_legacy_provider_version(helper: &crate::storage::usage_store::ProviderVersionInfo) -> bool {
    helper.provider_version.contains("ccusage") || helper.parser_version.contains("ccusage")
}
