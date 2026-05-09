use crate::{
    error::Result,
    paths::AppPaths,
    storage::{state::StateStore, usage_store::UsageStore},
    usage::{ccusage::CcusageCommandProvider, provider::UsageProvider},
};

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
    let result = CcusageCommandProvider::from_environment().poll(&mut usage_store)?;
    if result.diagnostics.is_empty() {
        println!("helpers: found");
        println!("provider command health: ok");
    } else {
        println!("helpers: not found or blocked");
        for diagnostic in &result.diagnostics {
            println!(
                "{}: {} - {}",
                diagnostic.provider_surface, diagnostic.code, diagnostic.message
            );
        }
        println!("No usage helper was found.");
        println!("Install the npm package with bundled helpers:");
        println!("  npm install -g glorp");
        println!("Or install helpers yourself and make sure these commands are on PATH:");
        println!("  ccusage");
        println!("  ccusage-codex");
    }

    for helper in usage_store.provider_versions()? {
        println!(
            "helper version: {} provider={} parser={}",
            helper.provider_surface, helper.provider_version, helper.parser_version
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
