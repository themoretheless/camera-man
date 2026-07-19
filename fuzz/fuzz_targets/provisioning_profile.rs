#![no_main]

use camera_man::provisioning_profile::{
    decoded_entitlement_strings, decoded_entitlements_grant, validate_decoded_profile,
};
use libfuzzer_sys::fuzz_target;

const BUNDLE_ID: &str = "com.cameraman.rust";
const REQUIRED: &str = "com.apple.developer.system-extension.install";
const GROUPS: &str = "com.apple.security.application-groups";

fuzz_target!(|input: &[u8]| {
    let _ = validate_decoded_profile(input, BUNDLE_ID, REQUIRED);
    let _ = decoded_entitlements_grant(input, REQUIRED);
    let groups = decoded_entitlement_strings(input, GROUPS);
    assert!(groups.iter().all(|group| !group.is_empty()));
});
