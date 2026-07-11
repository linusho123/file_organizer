//! BDD acceptance layer, Set A (original corpus, made to parse). The same
//! native binary must satisfy the unmodified behavioral contract.

mod support;

use std::path::PathBuf;

use gherkin_cargo_test::Features;
use support::organizer_steps;

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../features_original");
    Features::new(dir)
        .feature("classification", organizer_steps)
        .feature("collisions", organizer_steps)
        .feature("dry_run", organizer_steps)
        .feature("input_validation", organizer_steps)
        .feature("keep_structure", organizer_steps)
        .feature("recursive", organizer_steps)
        .feature("report", organizer_steps)
        .feature("undo", organizer_steps)
        .run()
}
