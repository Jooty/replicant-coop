use std::fs;
use std::path::Path;

use replicant_game::kaine::put_back_kaine_copies;
use replicant_proto::PartnerBody;

use crate::diagnostics::log_line;
use crate::player_body::real_body;

/// The co-op file to write in place of `story_rewrite` (or of `written` when that is None), with
/// the Kaine copy put back while this phase plays her only because of our seed swap.
pub(crate) fn guard_coop_save(
    coop_file: &Path,
    written: &[u8],
    story_rewrite: Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    let swapped_to_kaine =
        real_body().is_some_and(|real| real.by_swap && real.body == PartnerBody::SeedKaineE);
    if !swapped_to_kaine {
        return story_rewrite;
    }
    let on_disk = fs::read(coop_file).ok();
    let mut file = story_rewrite.clone().unwrap_or_else(|| written.to_vec());
    let put_back = put_back_kaine_copies(&mut file, on_disk.as_deref());
    if put_back.is_empty() {
        log_line("kaine: co-op save while playing her, no slot carries her copy, written as is");
        return story_rewrite;
    }
    for slot in &put_back {
        log_line(&format!(
            "kaine: slot {} her copy put back from {}{}",
            slot.slot,
            if slot.from_disk {
                "disk"
            } else {
                "zeros (no slot on disk)"
            },
            if slot.seed_index_reads_kaine {
                "; its seed index reads 4, which the swap never writes, left as is"
            } else {
                ""
            }
        ));
    }
    Some(file)
}
