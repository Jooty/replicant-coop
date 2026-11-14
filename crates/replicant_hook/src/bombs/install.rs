use crate::bombs::{blast_hook, wall_hook};
use crate::build_gate::VerifiedBuild;

pub fn install(build: VerifiedBuild) {
    wall_hook::install(build);
    blast_hook::install(build);
}
