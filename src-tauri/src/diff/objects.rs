use crate::schema::{SequenceInfo, ViewInfo};
use super::utils;

pub fn views_differ(local: &ViewInfo, remote: &ViewInfo) -> bool {
    // Normalize view definitions before comparison to handle formatting differences
    // (CREATE VIEW prefix in local vs just SELECT in remote, quoted identifiers, whitespace)
    let local_def_normalized = utils::normalize_view_definition(&local.definition);
    let remote_def_normalized = utils::normalize_view_definition(&remote.definition);
    
    // DEBUG: Print what we're comparing and find first difference
    if local_def_normalized != remote_def_normalized {
        eprintln!("=== VIEW DIFF DEBUG for {} ===", local.name);
        eprintln!("LOCAL length: {}, REMOTE length: {}", local_def_normalized.len(), remote_def_normalized.len());
        
        // Find first difference
        let local_chars: Vec<char> = local_def_normalized.chars().collect();
        let remote_chars: Vec<char> = remote_def_normalized.chars().collect();
        for (i, (l, r)) in local_chars.iter().zip(remote_chars.iter()).enumerate() {
            if l != r {
                let start = i.saturating_sub(20);
                let end = (i + 30).min(local_chars.len()).min(remote_chars.len());
                eprintln!("FIRST DIFF at position {}", i);
                eprintln!("LOCAL context: {:?}", &local_def_normalized[start..end.min(local_def_normalized.len())]);
                eprintln!("REMOTE context: {:?}", &remote_def_normalized[start..end.min(remote_def_normalized.len())]);
                break;
            }
        }
        if local_chars.len() != remote_chars.len() {
            eprintln!("LENGTH DIFFERS: local={} remote={}", local_chars.len(), remote_chars.len());
            let local_end: String = local_chars.iter().rev().take(50).collect::<Vec<_>>().into_iter().rev().collect();
            let remote_end: String = remote_chars.iter().rev().take(50).collect::<Vec<_>>().into_iter().rev().collect();
            eprintln!("LOCAL ENDING: {:?}", local_end);
            eprintln!("REMOTE ENDING: {:?}", remote_end);
        }
        eprintln!("=== END DEBUG ===");
    }
    
    local_def_normalized != remote_def_normalized
        || local.is_materialized != remote.is_materialized
        || local.with_options != remote.with_options
        || local.check_option != remote.check_option
}

pub fn sequences_differ(local: &SequenceInfo, remote: &SequenceInfo) -> bool {
    local.data_type != remote.data_type
        || local.start_value != remote.start_value
        || local.min_value != remote.min_value
        || local.max_value != remote.max_value
        || local.increment != remote.increment
        || local.cycle != remote.cycle
        || local.cache_size != remote.cache_size
}
