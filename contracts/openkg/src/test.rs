use super::*;
use ostd::mock::build_runtime;

#[test]
fn test() {
    let old_admin = get_admin();
    assert_eq!(CONTRACT_COMMON.admin(), &old_admin);

    let new_admin = Address::repeat_byte(1);
    let build = build_runtime();
    build.witness(&[CONTRACT_COMMON.admin().clone()]);
    assert!(update_admin(&new_admin));
    let old_admin = get_admin();
    assert_eq!(&old_admin, &new_admin)
}
