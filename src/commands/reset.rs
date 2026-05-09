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
    println!("glorp pet state reset; usage cache was left alone");
    Ok(())
}
