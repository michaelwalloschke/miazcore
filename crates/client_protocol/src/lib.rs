//! Engine-independent protocol boundary for the Learning Client.
//!
//! Ticket 12 establishes the dependency seam only. Login and world codecs,
//! cryptography, framing, and movement packets are intentionally deferred.

/// The only client build accepted by the World-entry Slice.
pub const TARGET_CLIENT_BUILD: u16 = 12_340;

#[cfg(test)]
mod tests {
    use super::TARGET_CLIENT_BUILD;

    #[test]
    fn target_build_is_the_locked_wrath_client_build() {
        assert_eq!(TARGET_CLIENT_BUILD, 12_340);
    }
}
