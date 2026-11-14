use std::sync::{Arc, Mutex};

use nc_launcher::app::NetForms;
use nc_launcher::net::{ConnectionReport, LauncherNet, NetError, NetState, NetStatus};

const FIXTURE_TICKET: &str = "nc1closingticket";

struct SlowToLetGoNet {
    state: Mutex<NetState>,
}

impl SlowToLetGoNet {
    fn hosting() -> Self {
        Self {
            state: Mutex::new(NetState::Hosting {
                ticket: FIXTURE_TICKET.to_owned(),
            }),
        }
    }

    fn let_go(&self) {
        *self.state.lock().expect("state") = NetState::Idle;
    }
}

impl LauncherNet for SlowToLetGoNet {
    fn host(&self) -> Result<String, NetError> {
        Err(NetError::Unavailable)
    }

    fn join(&self, _ticket: &str) -> Result<(), NetError> {
        Err(NetError::Unavailable)
    }

    fn test_connection(&self, _ticket: &str, _seconds: u32) -> Result<ConnectionReport, NetError> {
        Err(NetError::Unavailable)
    }

    fn leave(&self) {}

    fn status(&self) -> NetStatus {
        NetStatus {
            state: self.state.lock().expect("state").clone(),
        }
    }
}

#[test]
fn a_left_session_stays_closing_until_the_network_lets_go_of_it() {
    let net = Arc::new(SlowToLetGoNet::hosting());
    let mut forms = NetForms::new(net.clone());
    assert!(!forms.closing_session());
    forms.leave();
    forms.poll();
    assert!(
        forms.closing_session(),
        "the network still reports the lobby, but it is closing"
    );
    net.let_go();
    forms.poll();
    assert!(!forms.closing_session());
}
