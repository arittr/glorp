use crate::{
    error::{GlorpError, Result},
    paths::AppPaths,
    storage::{state::StateStore, usage_store::UsageStore},
    usage::{ccusage::CcusageCommandProvider, provider::UsageProvider},
};

pub fn run() -> Result<()> {
    let paths = AppPaths::resolve()?;
    let store = StateStore::new(paths.state_file);
    let Some(state) = store.load()? else {
        return Err(GlorpError::Message(
            "no glorp pet exists yet; run `glorp init` first".into(),
        ));
    };

    let mut provider_line = "provider: blocked".to_string();
    let mut recent_effective = 0.0;
    let mut diagnostic_line = None;
    if let Ok(mut usage_store) = UsageStore::open(&paths.usage_db) {
        match CcusageCommandProvider::from_environment().poll(&mut usage_store) {
            Ok(result) => {
                recent_effective = result.total_effective_tokens;
                if let Some(diagnostic) = result.diagnostics.first() {
                    provider_line = format!("provider: blocked ({})", diagnostic.code);
                    diagnostic_line = Some(format!("diagnostic: {}", diagnostic.code));
                } else {
                    provider_line = "provider: local-log-derived".into();
                }
            }
            Err(err) => {
                diagnostic_line = Some(format!("diagnostic: {err}"));
            }
        }
    }

    println!(
        "{} / {}",
        state.pet.accepted_name, state.pet.generated_species
    );
    println!("stage: {}  xp: {:.2}", state.stage, state.xp);
    println!(
        "vitals: fed {:.0} happy {:.0} energy {:.0}",
        state.vitals.fed, state.vitals.happiness, state.vitals.energy
    );
    println!(
        "effective tokens: today {:.0} recent {:.0} lifetime {:.0}",
        display_tokens(recent_effective),
        display_tokens(recent_effective),
        display_tokens(state.lifetime_effective_tokens)
    );
    println!("{provider_line}");
    println!("cost: local-derived display metadata only");
    if let Some(event) = state.recent_events.last() {
        println!("event: {event}");
    }
    if let Some(line) = diagnostic_line {
        println!("{line}");
    }
    Ok(())
}

fn display_tokens(value: f64) -> f64 {
    value.max(0.0)
}
