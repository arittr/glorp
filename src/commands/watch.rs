use crate::{
    error::{GlorpError, Result},
    paths::AppPaths,
    storage::state::StateStore,
    tui::{app::WatchApp, view_model::WatchViewModel},
};

pub fn run() -> Result<()> {
    let paths = AppPaths::resolve()?;
    let Some(state) = StateStore::new(paths.state_file).load()? else {
        return Err(GlorpError::Message(
            "no glorp pet exists yet; run `glorp init` first".into(),
        ));
    };
    let mut vm = WatchViewModel::fixture();
    vm.pet_name = state.pet.accepted_name;
    vm.species = state.pet.generated_species;
    vm.stage = state.stage;
    vm.fed = state.vitals.fed / 100.0;
    vm.happiness = state.vitals.happiness / 100.0;
    vm.energy = state.vitals.energy / 100.0;
    WatchApp::new(vm).run()
}
