use crate::cli::PreviewScenarioArg;
use crate::dev_preview::scenarios::{generate_preview_bundle, PreviewSelection};
use crate::error::Result;
use std::path::PathBuf;

pub fn run(out: PathBuf, scenario: PreviewScenarioArg) -> Result<()> {
    let selection = match scenario {
        PreviewScenarioArg::All => PreviewSelection::All,
        PreviewScenarioArg::Watch => PreviewSelection::Watch,
        PreviewScenarioArg::Pets => PreviewSelection::Pets,
    };

    generate_preview_bundle(&out, selection)?;
    println!("Wrote Glorp preview bundle to {}", out.display());
    Ok(())
}
