use replicant_proto::{EnemyId, EnemyIdList, MAX_ENEMY_ID_LIST_PARTS, MAX_ENEMY_IDS_PER_LIST_PART};

pub fn split_id_list(tick: u32, phase: u16, mut ids: Vec<EnemyId>) -> Vec<EnemyIdList> {
    ids.sort();
    ids.truncate(MAX_ENEMY_IDS_PER_LIST_PART * usize::from(MAX_ENEMY_ID_LIST_PARTS));
    let chunks: Vec<Vec<EnemyId>> = if ids.is_empty() {
        vec![Vec::new()]
    } else {
        ids.chunks(MAX_ENEMY_IDS_PER_LIST_PART)
            .map(<[EnemyId]>::to_vec)
            .collect()
    };
    let parts = u8::try_from(chunks.len()).unwrap_or(MAX_ENEMY_ID_LIST_PARTS);
    (0_u8..)
        .zip(chunks)
        .map(|(part, ids)| EnemyIdList {
            phase,
            tick,
            part,
            parts,
            ids,
        })
        .collect()
}
