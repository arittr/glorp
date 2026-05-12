use crate::{
    error::{GlorpError, Result},
    paths::AppPaths,
    storage::state::StateStore,
};

pub fn run(yes: bool) -> Result<()> {
    if !yes {
        return Err(GlorpError::Message(
            "reset requires confirmation; rerun with --yes".into(),
        ));
    }
    let paths = AppPaths::resolve()?;
    StateStore::new(paths.state_file).delete()?;
    // Also wipe the usage ledger and cursors. Without this, the next
    // `glorp init` re-derives calibration from old events that may have
    // been computed under a previous (buggier) effective-token formula,
    // and the watch view reads the same stale rows for "today" totals.
    if paths.usage_db.exists() {
        std::fs::remove_file(&paths.usage_db).map_err(|err| {
            GlorpError::Message(format!(
                "failed to remove usage cache at {}: {err}",
                paths.usage_db.display()
            ))
        })?;
    }
    println!("glorp pet state and usage cache reset");
    Ok(())
}
