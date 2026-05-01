//! Verifies that insta is wired up correctly. Not gated on `--ignored`
//! because no container is needed.

use serde_json::json;

#[test]
fn redacted_snapshot_is_stable() {
    let value = json!({
        "code": 0,
        "msg": "",
        "data": {
            "version": "v3.1.7",
            "container_id": "deadbeefcafebabe",
        }
    });

    insta::assert_yaml_snapshot!(value, {
        ".data.container_id" => "[redacted]",
    });
}
