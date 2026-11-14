use std::panic;

use replicant_game::pickups::{is_story_item, plan_partner_pickup, story_item_raise_amount};
use replicant_proto::Message;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::event_log::post_event;
use crate::net;
use crate::pickups::hook::add_item_for_partner;

pub fn tick(build: &VerifiedBuild) {
    let _ = panic::catch_unwind(|| crate::pickups::turn_in::report_finished_turn_in(build));
    let _ = panic::catch_unwind(crate::pickups::shared_items::tick);
    let Some(net) = net::handle() else {
        return;
    };
    let arrived = net.take_relayed(|message| matches!(message, Message::Pickup(_)));
    if arrived.is_empty() {
        return;
    }
    let name = net
        .view()
        .partner_name()
        .map(str::to_owned)
        .unwrap_or_default();
    for message in arrived {
        let Message::Pickup(pickup) = message else {
            continue;
        };
        let plan = plan_partner_pickup(&name, pickup);
        post_event(&plan.notice);
        log_line(&format!(
            "pickups: partner took item id {} x{}, grant {}",
            pickup.item_id,
            pickup.count,
            plan.grant.is_some()
        ));
        if is_story_item(pickup.item_id) {
            raise_story_item(pickup.item_id, pickup.count);
        } else if let Some(grant) = plan.grant {
            let result = panic::catch_unwind(|| add_item_for_partner(grant.item_id, grant.count))
                .ok()
                .flatten();
            log_line(&format!(
                "pickups: key item id {} granted, add returned {result:?}",
                grant.item_id
            ));
        }
    }
}

fn raise_story_item(item_id: u16, partner_count: u8) {
    let own = crate::pickups::item_calls::local_count(item_id).unwrap_or(0);
    let raise = story_item_raise_amount(partner_count, own);
    if raise > 0 {
        let _ = panic::catch_unwind(|| add_item_for_partner(item_id, raise));
    }
    let now = crate::pickups::item_calls::local_count(item_id).unwrap_or(0);
    log_line(&format!(
        "pickups: story item {item_id} now {now} here (partner has {partner_count})"
    ));
}
