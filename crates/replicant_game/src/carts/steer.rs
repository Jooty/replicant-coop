use crate::carts::{
    CART_FRAME_SET_GAP_FRAMES, CART_FRAME_SET_STOPS_FRAMES_BEFORE_END, CART_HOST_RUN_SILENT_MS,
    CART_HOST_RUN_STILL_FRAMES, CART_HOST_RUN_STILL_MS, CART_RUN_FRAMES_PER_SECOND_AT_RATE_ONE,
    CART_RUN_LONGEST_EXTRAPOLATION_MS, CART_STEER_RATE_PER_FRAME_OF_GAP, CART_STEERED_RATE_MAX,
    CART_STEERED_RATE_MIN,
};

/// The host's run of the cart this game follows, as its last progress message gave it. Times are
/// this game's clock at arrival; `moved_ms` is the last arrival whose frame differed from the one
/// before it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCartRun {
    pub seq: u32,
    pub frame: f32,
    pub rate: f32,
    pub received_ms: u64,
    pub moved_ms: u64,
}

/// Why the steering left this game's cart to its own game this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartLeftAlone {
    NoHostRide,
    HostSilent,
    HostStill,
    AtItsEnd,
}

/// What the guest's cart update does after the game's own: `gap` is the host's frame (carried
/// forward from its arrival at the host's rate) minus this cart's, in motion frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CartSteer {
    LeaveAlone(CartLeftAlone),
    Rate { rate: f32, gap: f32 },
    SetFrame { frame: f32, rate: f32, gap: f32 },
}

/// One tick of steering as the guest's cart update applied it, kept for the log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartSteerSample {
    pub own_frame: f32,
    pub newer_motion: bool,
    pub steer: CartSteer,
}

/// The latest progress the host sent for the ride this game follows; an older `seq` never
/// replaces a newer one, and nothing at or below an ended run's `seq` comes back.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HostCartRunTrack {
    latest: Option<HostCartRun>,
    ended_seq: Option<u32>,
}

impl HostCartRunTrack {
    pub const fn new() -> Self {
        Self {
            latest: None,
            ended_seq: None,
        }
    }

    pub fn clear(&mut self) {
        self.latest = None;
    }

    pub fn end(&mut self, seq: u32) {
        self.latest = None;
        self.ended_seq = Some(self.ended_seq.map_or(seq, |ended| ended.max(seq)));
    }

    pub fn latest(&self) -> Option<HostCartRun> {
        self.latest
    }

    pub fn receive(&mut self, seq: u32, frame: f32, rate: f32, now_ms: u64) -> bool {
        if self.ended_seq.is_some_and(|ended| seq <= ended) {
            return false;
        }
        let moved_ms = match self.latest {
            Some(last) if seq <= last.seq => return false,
            Some(last) if (frame - last.frame).abs() <= CART_HOST_RUN_STILL_FRAMES => last.moved_ms,
            _ => now_ms,
        };
        self.latest = Some(HostCartRun {
            seq,
            frame,
            rate,
            received_ms: now_ms,
            moved_ms,
        });
        true
    }
}

/// The steering rule for the guest's copy of a cart the host rides. A gap of more than
/// `CART_FRAME_SET_GAP_FRAMES` sets the frame, never to `end_frame` or past it, so the game's own
/// end test still fires; a smaller one is closed by the rate, which never reaches zero.
pub fn steer_cart(
    host: Option<&HostCartRun>,
    own_frame: f32,
    end_frame: f32,
    now_ms: u64,
) -> CartSteer {
    let Some(host) = host else {
        return CartSteer::LeaveAlone(CartLeftAlone::NoHostRide);
    };
    let age_ms = now_ms.saturating_sub(host.received_ms);
    if age_ms > CART_HOST_RUN_SILENT_MS {
        return CartSteer::LeaveAlone(CartLeftAlone::HostSilent);
    }
    // A host whose frame stopped (its game paused or out of focus) never holds this cart back.
    if now_ms.saturating_sub(host.moved_ms) > CART_HOST_RUN_STILL_MS {
        return CartSteer::LeaveAlone(CartLeftAlone::HostStill);
    }
    if own_frame >= end_frame {
        return CartSteer::LeaveAlone(CartLeftAlone::AtItsEnd);
    }
    let carried_seconds = age_ms.min(CART_RUN_LONGEST_EXTRAPOLATION_MS) as f32 / 1000.0;
    let target = host.frame + host.rate * CART_RUN_FRAMES_PER_SECOND_AT_RATE_ONE * carried_seconds;
    let gap = target - own_frame;
    let last_settable = end_frame - CART_FRAME_SET_STOPS_FRAMES_BEFORE_END;
    let frame = target.min(last_settable).max(0.0);
    if gap.abs() > CART_FRAME_SET_GAP_FRAMES
        && (frame - own_frame).abs() > CART_FRAME_SET_GAP_FRAMES
    {
        return CartSteer::SetFrame {
            frame,
            rate: steered_rate(host.rate),
            gap,
        };
    }
    CartSteer::Rate {
        rate: steered_rate(host.rate + CART_STEER_RATE_PER_FRAME_OF_GAP * gap),
        gap,
    }
}

fn steered_rate(rate: f32) -> f32 {
    if rate.is_nan() {
        return 1.0;
    }
    rate.clamp(CART_STEERED_RATE_MIN, CART_STEERED_RATE_MAX)
}
