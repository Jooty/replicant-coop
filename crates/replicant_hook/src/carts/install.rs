use crate::build_gate::VerifiedBuild;
use crate::carts::board_gate::install_board_gate;
use crate::carts::steer_hook::install_steering;

pub fn install(build: VerifiedBuild) {
    install_board_gate(build);
    install_steering(build);
}
