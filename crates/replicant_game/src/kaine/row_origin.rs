/// Where a companion table row comes from: written by hand after a run, or generated from the
/// game's script packs. A hand-made row wins where both cover the same place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowOrigin {
    HandMade,
    FromScripts,
}
