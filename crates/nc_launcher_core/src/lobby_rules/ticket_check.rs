use crate::INVALID_TICKET_TEXT;

/// What can be told about a pasted ticket before connecting: whether it reads as a host's address at all.
pub fn ticket_check(text: &str) -> Result<(), &'static str> {
    if nc_net::invite_reads(text.trim()) {
        Ok(())
    } else {
        Err(INVALID_TICKET_TEXT)
    }
}
