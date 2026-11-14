use std::sync::Arc;
use std::time::{Duration, Instant};

use nc_launcher::saves::{
    SlotSummary, coop_campaigns_path, coop_gamedata_path, read_campaign_seat, read_campaigns,
    read_hidden_copy, read_unsent, seat_backups_dir,
};
use nc_launcher::seat_round::{GuestSeatRound, HostSeatRound, HostStartStep};
use nc_launcher_core::{
    LauncherConfig, LauncherCore, LauncherEvent, LauncherNet, NetState,
    SEAT_DELIVERY_TIMEOUT_SECONDS, StartGateStep, TransportFactory,
};
use nc_net::{LoopbackNetwork, Transport};
use replicant_game::story_progress::{
    SLOT_BYTES, SLOTS_FILE_OFFSET, fix_header_checksum, seat_bytes_of,
};

use crate::scratch::fresh_dir;
use crate::seat_files::{seat_with_play, template_file};

const WAIT_LIMIT: Duration = Duration::from_secs(20);
const HOST_SLOT: u8 = 0;

fn loopback_factory(network: &LoopbackNetwork) -> TransportFactory {
    let network = network.clone();
    Arc::new(move || Ok(Box::new(network.endpoint()) as Box<dyn Transport>))
}

struct Side<R> {
    core: LauncherCore,
    round: R,
    events: Vec<LauncherEvent>,
}

struct Pair {
    host: Side<HostSeatRound>,
    guest: Side<GuestSeatRound>,
    steps: Vec<StartGateStep>,
}

impl Pair {
    fn pump(&mut self) {
        for event in self.host.core.drain_events() {
            if let Some(step) = self.host.round.on_event(&self.host.core, &event) {
                self.steps.push(step);
            }
            self.host.events.push(event);
        }
        for event in self.guest.core.drain_events() {
            self.guest.round.on_event(&self.guest.core, &event);
            self.guest.events.push(event);
        }
    }

    fn wait_for(&mut self, mut condition: impl FnMut(&Self) -> bool) -> bool {
        let deadline = Instant::now() + WAIT_LIMIT;
        while Instant::now() < deadline {
            self.pump();
            if condition(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(3));
        }
        false
    }

    fn connect(&mut self) {
        let ticket = self.host.core.host().expect("ticket");
        self.guest.core.join(&ticket).expect("join");
        assert!(self.wait_for(|pair| matches!(
            pair.host.core.status().state,
            NetState::Connected { .. }
        ) && pair.guest.core.link_status().partner().is_some()));
    }

    fn guest_member_slot(&self) -> u8 {
        self.host
            .core
            .link_status()
            .partner()
            .map(|partner| partner.slot)
            .expect("a guest")
    }
}

struct Folders {
    host_saves: std::path::PathBuf,
    guest_folder: std::path::PathBuf,
    guest_backups: std::path::PathBuf,
    template: std::path::PathBuf,
}

fn folders(name: &str) -> Folders {
    let root = fresh_dir(name);
    let host_saves = root.join("host/My Games");
    let steam = host_saves.join("Steam/76561198000000000");
    std::fs::create_dir_all(&steam).unwrap();
    let mut host_file = template_file();
    host_file[SLOTS_FILE_OFFSET..SLOTS_FILE_OFFSET + SLOT_BYTES]
        .copy_from_slice(&seat_with_play("Jay", 500.0, 1));
    host_file[0x6C] = 1;
    fix_header_checksum(&mut host_file).unwrap();
    std::fs::write(steam.join("GAMEDATA.coop"), host_file).unwrap();
    let guest_folder = root.join("guest/saves");
    std::fs::create_dir_all(&guest_folder).unwrap();
    let template = guest_folder.join("GAMEDATA");
    std::fs::write(&template, template_file()).unwrap();
    Folders {
        host_saves,
        guest_backups: seat_backups_dir(&root.join("guest/data")),
        guest_folder,
        template,
    }
}

fn pair(network: &LoopbackNetwork, folders: &Folders) -> Pair {
    let core = |name: &str| {
        LauncherCore::new(LauncherConfig::new(name), loopback_factory(network)).expect("core")
    };
    Pair {
        host: Side {
            core: core("Hosty"),
            round: HostSeatRound::new(coop_gamedata_path(&folders.host_saves).unwrap()),
            events: Vec::new(),
        },
        guest: Side {
            core: core("Guesty"),
            round: GuestSeatRound::new(
                Some(folders.guest_folder.clone()),
                vec![folders.template.clone()],
                folders.guest_backups.clone(),
            ),
            events: Vec::new(),
        },
        steps: Vec::new(),
    }
}

fn host_slot() -> SlotSummary {
    SlotSummary {
        index: HOST_SLOT,
        used: true,
        name: "Jay".to_owned(),
        phase: "P".to_owned(),
        level: 4,
        play_seconds: 500.0,
    }
}

fn guest_game_saves(path: &std::path::Path, seat: &[u8]) {
    let mut file = std::fs::read(path).unwrap();
    file[SLOTS_FILE_OFFSET..SLOTS_FILE_OFFSET + SLOT_BYTES].copy_from_slice(seat);
    fix_header_checksum(&mut file).unwrap();
    std::fs::write(path, file).unwrap();
}

fn start(pair: &mut Pair) {
    let guest_slot = pair.guest_member_slot();
    let step = pair
        .host
        .round
        .begin_start(&pair.host.core, guest_slot, &host_slot(), 0.0);
    assert_eq!(step, HostStartStep::Waiting);
    assert!(pair.wait_for(|pair| !pair.steps.is_empty()));
    assert_eq!(pair.steps.pop(), Some(StartGateStep::Launch));
}

fn stored_seat_play(folders: &Folders) -> Option<f64> {
    read_campaigns(&coop_campaigns_path(&folders.host_saves).unwrap())
        .get(&HOST_SLOT)?
        .seat
        .as_ref()
        .map(|seat| seat.play_seconds)
}

#[test]
fn a_seat_goes_to_the_guest_comes_back_after_a_save_and_the_newer_one_returns_next_session() {
    let network = LoopbackNetwork::new();
    let folders = folders("seat_round_trip");
    let coop = coop_gamedata_path(&folders.host_saves).unwrap();
    let mut pair = pair(&network, &folders);
    pair.connect();
    pair.host.round.pick(&pair.host.core, &host_slot());
    let campaign_id = read_campaigns(&coop_campaigns_path(&folders.host_saves).unwrap())
        [&HOST_SLOT]
        .id
        .clone();
    let first = seat_with_play("Guest", 100.0, 2);
    nc_launcher::saves::store_campaign_seat(
        &coop,
        HOST_SLOT,
        &first,
        nc_launcher::saves::seat_summary_of(&first).unwrap(),
        1,
    )
    .unwrap();
    pair.host.round.pick(&pair.host.core, &host_slot());
    assert!(pair.wait_for(|pair| pair.guest.round.seat_expected()));

    start(&mut pair);
    let session = folders.guest_folder.join(
        pair.guest
            .round
            .session_file_name()
            .expect("the session file is written before the start"),
    );
    assert_eq!(
        seat_bytes_of(&std::fs::read(&session).unwrap(), 0).unwrap(),
        first
    );

    let saved = seat_with_play("Guest", 200.0, 3);
    guest_game_saves(&session, &saved);
    pair.guest.round.on_event(
        &pair.guest.core,
        &LauncherEvent::GameSaved { succeeded: true },
    );
    assert!(pair.wait_for(|_| stored_seat_play(&folders) == Some(200.0)));
    assert_eq!(
        read_campaign_seat(&coop, HOST_SLOT).unwrap(),
        Some(saved.clone())
    );
    assert!(pair.wait_for(
        |_| read_hidden_copy(&folders.guest_backups, &campaign_id) == Some(saved.clone())
    ));
    assert!(read_unsent(&folders.guest_backups, &campaign_id).is_none());
    assert!(pair.wait_for(|pair| {
        pair.guest
            .round
            .card()
            .and_then(|card| card.second_seat.as_ref())
            .is_some_and(|seat| seat.summary.play_seconds == 200.0)
    }));

    pair.host.core.leave();
    assert!(pair.wait_for(|pair| matches!(pair.guest.core.status().state, NetState::Failed(_))));
    let newest = seat_with_play("Guest", 300.0, 4);
    guest_game_saves(&session, &newest);
    pair.guest.round.on_event(
        &pair.guest.core,
        &LauncherEvent::GameSaved { succeeded: true },
    );
    pair.guest.round.flush(&pair.guest.core);
    pair.pump();
    assert_eq!(
        read_unsent(&folders.guest_backups, &campaign_id),
        Some(newest.clone())
    );
    assert_eq!(stored_seat_play(&folders), Some(200.0));
    drop(pair);

    let mut next = self::pair(&network, &folders);
    next.connect();
    next.host.round.pick(&next.host.core, &host_slot());
    assert!(next.wait_for(|pair| pair.guest.round.seat_expected()));
    start(&mut next);
    assert_eq!(
        seat_bytes_of(&std::fs::read(&session).unwrap(), 0).unwrap(),
        newest
    );
    assert!(next.wait_for(|_| stored_seat_play(&folders) == Some(300.0)));
    assert_eq!(
        read_campaign_seat(&coop, HOST_SLOT).unwrap(),
        Some(newest.clone())
    );
    assert!(next.wait_for(
        |_| read_hidden_copy(&folders.guest_backups, &campaign_id) == Some(newest.clone())
    ));
    assert!(read_unsent(&folders.guest_backups, &campaign_id).is_none());
}

#[test]
fn a_seat_that_cannot_be_sent_or_never_arrives_stops_the_start() {
    let network = LoopbackNetwork::new();
    let folders = folders("seat_round_never_arrives");
    let coop = coop_gamedata_path(&folders.host_saves).unwrap();
    let mut pair = pair(&network, &folders);
    pair.connect();
    pair.host.round.pick(&pair.host.core, &host_slot());
    let seat = seat_with_play("Guest", 100.0, 2);
    nc_launcher::saves::store_campaign_seat(
        &coop,
        HOST_SLOT,
        &seat,
        nc_launcher::saves::seat_summary_of(&seat).unwrap(),
        1,
    )
    .unwrap();
    let guest_slot = pair.guest_member_slot();
    assert_eq!(
        pair.host
            .round
            .begin_start(&pair.host.core, guest_slot, &host_slot(), 0.0),
        HostStartStep::Waiting
    );
    assert!(pair.host.round.waiting_for_seat());
    assert!(matches!(
        pair.host.round.tick(SEAT_DELIVERY_TIMEOUT_SECONDS),
        Some(StartGateStep::Stop(reason)) if reason.contains("did not arrive within")
    ));
    assert!(!pair.host.round.waiting_for_seat());

    pair.guest.core.leave();
    assert!(pair.wait_for(|pair| pair.host.core.link_status().partner().is_none()));
    assert!(matches!(
        pair.host.round.begin_start(&pair.host.core, guest_slot, &host_slot(), 0.0),
        HostStartStep::Stop(reason) if reason.contains("not connected")
    ));
}
