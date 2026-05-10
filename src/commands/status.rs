use crate::{
    error::{GlorpError, Result},
    game::runtime::apply_usage_poll,
    paths::AppPaths,
    storage::{state::StateStore, usage_store::UsageStore},
    usage::{ccusage::CcusageCommandProvider, provider::UsageProvider},
};
use time::OffsetDateTime;

pub fn run() -> Result<()> {
    let paths = AppPaths::resolve()?;
    let store = StateStore::new(paths.state_file);
    let Some(mut state) = store.load()? else {
        return Err(GlorpError::Message(
            "no glorp pet exists yet; run `glorp init` first".into(),
        ));
    };

    let mut provider_line = "provider: blocked".to_string();
    let mut provider_health_line = "provider health: blocked".to_string();
    let mut usage_confidence = "estimated".to_string();
    let mut recent_effective = 0.0;
    let mut today_effective = 0.0;
    let mut diagnostic_line = None;
    if let Ok(mut usage_store) = UsageStore::open(&paths.usage_db) {
        let config = crate::config::AppConfig::load_or_default(&paths.config_file)?;
        let weights = crate::game::effective_tokens::EffectiveTokenWeights::from_config(config);
        match CcusageCommandProvider::from_environment_with_weights(weights).poll(&mut usage_store)
        {
            Ok(result) => {
                if !result.deltas.is_empty() || result.diagnostics.is_empty() {
                    let update = apply_usage_poll(
                        &mut state,
                        &mut usage_store,
                        &result,
                        OffsetDateTime::now_utc(),
                    )?;
                    store.save(&state)?;
                    recent_effective = update.recent_effective_tokens;
                }
                today_effective = usage_store.today_effective_tokens().unwrap_or(0.0);
                if let Some(diagnostic) = result.diagnostics.first() {
                    provider_line = format!("provider: blocked ({})", diagnostic.code);
                    provider_health_line =
                        format!("provider health: blocked ({})", diagnostic.code);
                    diagnostic_line = Some(format!("diagnostic: {}", diagnostic.code));
                } else {
                    provider_line = "provider: local-log-derived".into();
                    provider_health_line = "provider health: ok".into();
                    usage_confidence = "local-log-derived".into();
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
    println!("{}", stage_progress_line(state.xp));
    println!(
        "vitals: fed {:.0} happy {:.0} energy {:.0}",
        state.vitals.fed, state.vitals.happiness, state.vitals.energy
    );
    println!(
        "effective tokens ({usage_confidence}): today {:.0} recent {:.0} lifetime {:.0}",
        display_tokens(today_effective),
        display_tokens(recent_effective),
        display_tokens(state.lifetime_effective_tokens)
    );
    println!("{provider_line}");
    println!("{provider_health_line}");
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

fn stage_progress_line(xp: f64) -> String {
    const THRESHOLDS: [f64; 7] = [0.0, 0.25, 1.0, 3.0, 7.0, 21.0, 49.0];
    let xp = xp.max(0.0);
    let current = THRESHOLDS
        .iter()
        .rposition(|threshold| xp >= *threshold)
        .unwrap_or(0);

    if current >= THRESHOLDS.len() - 1 {
        return "stage progress: s6 complete".into();
    }

    let start = THRESHOLDS[current];
    let next = THRESHOLDS[current + 1];
    let progress = if next > start {
        ((xp - start) / (next - start) * 100.0).clamp(0.0, 100.0)
    } else {
        100.0
    };

    format!(
        "stage progress: s{} {:.0}% toward s{}",
        current,
        progress,
        current + 1
    )
}
