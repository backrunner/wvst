use std::ffi::c_void;
use std::ptr;

use super::*;

#[test]
fn exposes_component_handler_interface() {
    let mut handler = Vst3ComponentHandler::new();
    let iid = parse_tuid_hex(VST3_I_COMPONENT_HANDLER_IID).expect("iid");
    let mut object = ptr::null_mut();

    let result = unsafe {
        ((*(*handler.as_mut_ptr()).vtable).query_interface)(
            handler.as_mut_ptr(),
            iid.as_ptr().cast(),
            &mut object,
        )
    };

    assert_eq!(result, K_RESULT_OK);
    assert_eq!(object, handler.as_mut_ptr().cast::<c_void>());
}

#[test]
fn exposes_component_handler2_interface() {
    let mut handler = Vst3ComponentHandler::new();
    let iid = parse_tuid_hex(VST3_I_COMPONENT_HANDLER2_IID).expect("iid");
    let mut object = ptr::null_mut();

    let result = unsafe {
        ((*(*handler.as_mut_ptr()).vtable).query_interface)(
            handler.as_mut_ptr(),
            iid.as_ptr().cast(),
            &mut object,
        )
    };

    assert_eq!(result, K_RESULT_OK);
    assert!(!object.is_null());

    let handler2 = object.cast::<IComponentHandler2>();
    let mut v1_object = ptr::null_mut();
    let v1_iid = parse_tuid_hex(VST3_I_COMPONENT_HANDLER_IID).expect("v1 iid");

    let result = unsafe {
        ((*(*handler2).vtable).query_interface)(handler2, v1_iid.as_ptr().cast(), &mut v1_object)
    };

    assert_eq!(result, K_RESULT_OK);
    assert_eq!(v1_object, handler.as_mut_ptr().cast::<c_void>());
}

#[test]
fn records_edit_and_restart_callbacks() {
    let mut handler = Vst3ComponentHandler::new();
    let pointer = handler.as_mut_ptr();

    unsafe {
        ((*(*pointer).vtable).begin_edit)(pointer, 42);
        ((*(*pointer).vtable).perform_edit)(pointer, 42, 0.75);
        ((*(*pointer).vtable).end_edit)(pointer, 42);
        ((*(*pointer).vtable).restart_component)(pointer, 3);
    }

    let snapshot = handler.snapshot();
    assert_eq!(snapshot.total_events, 4);
    assert_eq!(snapshot.recent_events.len(), 4);
    assert_eq!(
        snapshot.recent_events[0].kind,
        Vst3ComponentHandlerEventKind::BeginEdit
    );
    assert_eq!(snapshot.recent_events[1].parameter_id, Some(42));
    assert_eq!(snapshot.recent_events[1].value_normalized, Some(0.75));
    assert_eq!(
        snapshot.recent_events[3].kind,
        Vst3ComponentHandlerEventKind::RestartComponent
    );
    assert_eq!(snapshot.recent_events[3].flags, Some(3));
}

#[test]
fn records_component_handler2_callbacks() {
    let mut handler = Vst3ComponentHandler::new();
    let iid = parse_tuid_hex(VST3_I_COMPONENT_HANDLER2_IID).expect("iid");
    let mut object = ptr::null_mut();
    let editor = std::ffi::CString::new("editor").expect("editor name");

    unsafe {
        ((*(*handler.as_mut_ptr()).vtable).query_interface)(
            handler.as_mut_ptr(),
            iid.as_ptr().cast(),
            &mut object,
        );
        let handler2 = object.cast::<IComponentHandler2>();
        ((*(*handler2).vtable).set_dirty)(handler2, 1);
        ((*(*handler2).vtable).request_open_editor)(handler2, editor.as_ptr());
        ((*(*handler2).vtable).start_group_edit)(handler2);
        ((*(*handler2).vtable).finish_group_edit)(handler2);
    }

    let snapshot = handler.snapshot();
    assert_eq!(snapshot.total_events, 4);
    assert_eq!(
        snapshot.recent_events[0].kind,
        Vst3ComponentHandlerEventKind::SetDirty
    );
    assert_eq!(snapshot.recent_events[0].dirty, Some(true));
    assert_eq!(
        snapshot.recent_events[1].kind,
        Vst3ComponentHandlerEventKind::RequestOpenEditor
    );
    assert_eq!(
        snapshot.recent_events[1].editor_name.as_deref(),
        Some("editor")
    );
    assert_eq!(
        snapshot.recent_events[2].kind,
        Vst3ComponentHandlerEventKind::StartGroupEdit
    );
    assert_eq!(
        snapshot.recent_events[3].kind,
        Vst3ComponentHandlerEventKind::FinishGroupEdit
    );
}
