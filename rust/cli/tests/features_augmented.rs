//! BDD acceptance layer, Set B (Set A + edge_behaviors documenting the five
//! previously-undocumented behaviors). Same binary, a strictly larger contract.

mod support;

use std::path::PathBuf;

use gherkin_cargo_test::Features;
use support::organizer_steps;

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../features_augmented");
    Features::new(dir)
        .feature("classification", organizer_steps)
        .feature("collisions", organizer_steps)
        .feature("dry_run", organizer_steps)
        .feature("edge_behaviors", organizer_steps)
        .feature("input_validation", organizer_steps)
        .feature("keep_structure", organizer_steps)
        .feature("recursive", organizer_steps)
        .feature("report", organizer_steps)
        .feature("undo", organizer_steps)
        .run()
}
