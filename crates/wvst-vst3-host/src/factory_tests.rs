use super::*;
use crate::HostError;

#[test]
fn rejects_invalid_component_class_id_before_loading_bundle() {
    let error =
        create_vst3_component_probe("/tmp/Missing.vst3", "class-a").expect_err("invalid class id");

    assert!(matches!(error, HostError::InvalidClassId(class_id) if class_id == "class-a"));
}

#[test]
fn rejects_invalid_runtime_class_id_before_loading_bundle() {
    let config = crate::Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let error = match create_vst3_component_instance("/tmp/Missing.vst3", "class-a", config) {
        Ok(_) => panic!("expected invalid class id"),
        Err(error) => error,
    };

    assert!(matches!(error, HostError::InvalidClassId(class_id) if class_id == "class-a"));
}
