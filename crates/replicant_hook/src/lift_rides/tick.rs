use replicant_game::stay_together::FollowRole;
use replicant_proto::Message;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::lift_rides::guest::{on_answer, tick_guest};
use crate::lift_rides::host::{on_call, tick_host};
use crate::net;

pub(crate) fn tick(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let host = FollowRole::from_slot(net.view().local_slot) == FollowRole::Host;
    let wanted =
        |message: &Message| matches!(message, Message::LiftCall(_) | Message::LiftCallAnswer(_));
    for message in net.take_relayed(wanted) {
        match message {
            Message::LiftCall(call) if host => on_call(build, call),
            Message::LiftCallAnswer(answer) if !host => on_answer(answer),
            Message::LiftCall(_) | Message::LiftCallAnswer(_) => {
                log_line("lift ride: a lift message for the other role arrived, ignored");
            }
            _ => {}
        }
    }
    if host {
        tick_host(build);
    } else {
        tick_guest(build);
    }
}
