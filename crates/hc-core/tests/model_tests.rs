use hc_core::model::*;

#[test]
fn test_value_display() {
    assert_eq!(Value::U16(42).to_string(), "42");
    assert_eq!(Value::F32(3.14).to_string(), "3.14");
    assert_eq!(Value::Bool(true).to_string(), "true");
}

#[test]
fn test_project_default() {
    let p = Project::default();
    assert_eq!(p.name, "新工程");
    assert_eq!(p.version, 1);
    assert_eq!(p.runtime.default_poll_interval_ms, 1000);
}

#[test]
fn test_sample_into_tag_update() {
    let s = Sample {
        tag_id: "dev/t".into(),
        ts: chrono::Utc::now(),
        value: Value::F32(25.5),
        quality: Quality::Good,
    };
    let u: TagUpdate = (&s).into();
    assert_eq!(u.tag_id, "dev/t");
    assert_eq!(u.value, Value::F32(25.5));
}

#[test]
fn test_quality_ordering() {
    assert_eq!(Quality::Good as i32, 0);
    assert_eq!(Quality::Bad as i32, 1);
    assert_eq!(Quality::Stale as i32, 2);
}

#[test]
fn test_device_default() {
    let d = Device::default();
    assert_eq!(d.id, "default");
    assert!(d.enabled);
}

#[test]
fn test_tag_default() {
    let t = Tag::default();
    assert_eq!(t.data_type, DataType::U16);
}

#[test]
fn test_serde_roundtrip() {
    let v = Value::F32(42.5);
    let json = serde_json::to_string(&v).unwrap();
    let back: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}
