use wg_2024::tests::*;
use crate::KrustyCrapDrone;
use super::test_flooding::generic_flood_request_forward;

#[cfg(test)]

#[test]
fn test_generic_fragment_forward() {
    generic_fragment_forward::<KrustyCrapDrone>();
}

#[test]
fn test_generic_fragment_drop() {
    generic_fragment_drop::<KrustyCrapDrone>();
}

#[test]
fn test_generic_chain_fragment_drop() {
    generic_chain_fragment_drop::<KrustyCrapDrone>();
}

#[test]
fn test_generic_chain_fragment_ack() {
    generic_chain_fragment_ack::<KrustyCrapDrone>();
}

#[test]
fn test_generic_flood_request_forward() {
    generic_flood_request_forward::<KrustyCrapDrone>();
}
