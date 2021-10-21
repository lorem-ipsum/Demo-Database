///
///  FD_FIELD=2     PAGE_FIELD=8
/// ____________:____________________
///
pub const FD_FIELD: usize = 1;
pub const PAGE_FIELD: usize = 6;

// How many bytes a page contains
pub const PAGE_SIZE: usize = 1024;

// How many pages the cache holds
pub const BUF_SIZE: usize = 1 << (FD_FIELD + PAGE_FIELD);
