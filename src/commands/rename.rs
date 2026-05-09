use crate::{
    error::{GlorpError, Result},
    paths::AppPaths,
    storage::state::StateStore,
};

pub fn run(name: String) -> Result<()> {
    let paths = AppPaths::resolve()?;
    let store = StateStore::new(paths.state_file);
    let Some(mut state) = store.load()? else {
        return Err(GlorpError::Message(
            "no glorp pet exists yet; run `glorp init` first".into(),
        ));
    };
    state.pet.accepted_name = name.trim().to_string();
    store.save(&state)?;
    println!("renamed pet to {}", state.pet.accepted_name);
    Ok(())
}
