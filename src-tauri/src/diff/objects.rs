use crate::schema::{SequenceInfo, ViewInfo};

pub fn views_differ(local: &ViewInfo, remote: &ViewInfo) -> bool {
    local.definition != remote.definition
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
