/// A fill part way across: `amount` of the width, growing from the left while `filling`, and giving way from
/// the left while emptying.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sweep {
    pub amount: f32,
    pub filling: bool,
}

impl Sweep {
    pub const EMPTY: Self = Self {
        amount: 0.0,
        filling: false,
    };
    pub const FULL: Self = Self {
        amount: 1.0,
        filling: true,
    };

    /// The part of `left..right` the fill covers.
    pub fn covered(self, left: f32, right: f32) -> (f32, f32) {
        let width = (right - left) * self.amount.clamp(0.0, 1.0);
        if self.filling {
            (left, left + width)
        } else {
            (right - width, right)
        }
    }

    pub fn is_full(self) -> bool {
        self.amount >= 1.0
    }

    pub fn is_empty(self) -> bool {
        self.amount <= 0.0
    }
}
