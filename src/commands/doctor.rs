use crate::{
    error::Result,
    paths::AppPaths,
    storage::{state::StateStore, usage_store::UsageStore},
    usage::{agentsview::AgentsviewCommandProvider, provider::UsageProvider},
};

pub fn run(refresh_usage_snapshots: bool) -> Result<()> {
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
    let diagnostics = if refresh_usage_snapshots {
        refresh_usage_snapshots_for_doctor(&mut usage_store, &provider)?
    } else {
        provider.poll(&mut usage_store)?.diagnostics
    };
    println!("provider: agentsview");
    if diagnostics.is_empty() {
        println!("helpers: found");
        println!("provider command health: ok");
        println!("required usage helper: found");
        if let Some(version) = provider.discovered_version() {
            println!("helper version: agentsview provider={version} parser={version}");
        }
    } else {
        println!("helpers: not found or blocked");
        println!("required usage helper: blocked");
        for diagnostic in &diagnostics {
            println!(
                "{}: {} - {}",
                diagnostic.provider_surface, diagnostic.code, diagnostic.message
            );
        }
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_helper")
        {
            println!("Default provider blocked.");
            println!(
                "Install agentsview and make it available on PATH, or set GLORP_AGENTSVIEW_BIN."
            );
        }
    }

    let provider_versions = usage_store.provider_versions()?;
    for helper in provider_versions {
        println!(
            "helper version: {} provider={} parser={}",
            helper.provider_surface, helper.provider_version, helper.parser_version
        );
    }

    let now = crate::time::usage_now_utc();
    let provider_day = crate::usage::day_axis::tokenmaxxing_provider_day(now);
    if let Some(source_totals) = usage_store
        .snapshot_totals_by_source_for_provider_day(provider_day)?
        .value
    {
        for source in source_totals.sources {
            println!(
                "provider source: {} today={:.0}",
                source.accounting_source,
                source.total_tokens.max(0.0)
            );
        }
    }

    let recent_sources = usage_store
        .canonical_total_tokens_by_source_between(now - time::Duration::hours(24), now)
        .unwrap_or_default();
    for (source, total) in recent_sources {
        println!("source: {} recent_24h={:.0}", source, total.max(0.0));
    }

    for correction in usage_store.recent_provider_corrections(5)? {
        println!(
            "recent correction: {} {} previous={:.0} current={:.0} decreased={:.0}",
            correction.accounting_source,
            correction.provider_day,
            correction.previous_total_tokens.max(0.0),
            correction.current_total_tokens.max(0.0),
            correction.decrease_tokens.max(0.0)
        );
    }

    for diagnostic in usage_store.recent_diagnostics(5)? {
        println!(
            "recent diagnostic: {} {} {}",
            diagnostic.provider_surface, diagnostic.code, diagnostic.message
        );
    }
    Ok(())
}

fn refresh_usage_snapshots_for_doctor(
    usage_store: &mut UsageStore,
    provider: &AgentsviewCommandProvider,
) -> Result<Vec<crate::usage::provider::ProviderDiagnostic>> {
    let now = crate::time::usage_now_utc();
    let provider_day = crate::usage::day_axis::tokenmaxxing_provider_day(now);
    let before = usage_store.snapshot_totals_for_provider_day(provider_day)?;
    let diagnostics = provider.refresh_snapshots_only(usage_store)?;
    let after = usage_store.snapshot_totals_for_provider_day(provider_day)?;

    println!("refresh usage snapshots: requested {provider_day}");
    println!("before provider today: {}", format_snapshot_total(&before));
    println!("after provider today: {}", format_snapshot_total(&after));
    if diagnostics.is_empty() {
        println!("blocked provider scopes: none");
    } else {
        for diagnostic in &diagnostics {
            println!(
                "blocked provider scope: {} {}",
                diagnostic.provider_surface, diagnostic.code
            );
        }
    }
    println!("pet state unchanged");
    Ok(diagnostics)
}

fn format_snapshot_total(
    snapshot: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
) -> String {
    snapshot
        .value
        .as_ref()
        .map(|totals| format!("{:.0}", totals.total_tokens))
        .unwrap_or_else(|| format!("{:?}", snapshot.state))
}
