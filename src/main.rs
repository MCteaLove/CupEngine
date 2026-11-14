mod editor;
mod inspector;
mod project;
mod viewport;

use engine::prelude::*;

fn main() {
    run(
        AppConfig { title: "CupEngine Editor".into(), width: 1600, height: 900, fixed_hz: 120.0 },
        editor::Editor::new(),
    );
}
