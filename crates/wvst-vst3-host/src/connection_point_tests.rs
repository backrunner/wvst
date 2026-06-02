use std::ffi::c_void;
use std::ptr;

use crate::Vst3HostMessage;
use crate::vst3_abi::{
    IConnectionPoint, IConnectionPointVTable, IMessage, K_RESULT_OK, TUid, VST3_FUNKNOWN_IID,
    VST3_I_CONNECTION_POINT_IID, parse_tuid_hex,
};

use super::*;

#[test]
fn connects_disconnects_and_releases_connection_points() {
    let mut left = FakeConnectionPoint::new();
    let mut right = FakeConnectionPoint::new();

    let left_point =
        unsafe { Vst3ConnectionPoint::from_raw(left.raw_connection_point()) }.expect("left");
    let right_point =
        unsafe { Vst3ConnectionPoint::from_raw(right.raw_connection_point()) }.expect("right");

    left_point.connect(&right_point).expect("connect");
    left_point.disconnect(&right_point).expect("disconnect");
    drop(left_point);
    drop(right_point);

    assert_eq!(left.connect_calls, 1);
    assert_eq!(left.disconnect_calls, 1);
    assert!(!left.last_other.is_null());
    assert_eq!(left.release_calls, 1);
    assert_eq!(right.release_calls, 1);
}

#[test]
fn reports_failed_connection_calls() {
    let mut left = FakeConnectionPoint::new();
    let mut right = FakeConnectionPoint::new();
    left.connect_result = -19;

    let left_point =
        unsafe { Vst3ConnectionPoint::from_raw(left.raw_connection_point()) }.expect("left");
    let right_point =
        unsafe { Vst3ConnectionPoint::from_raw(right.raw_connection_point()) }.expect("right");

    assert_eq!(
        left_point
            .connect(&right_point)
            .expect_err("connect failure"),
        HostError::ConnectionPointCallFailed {
            method: "connect",
            result: -19
        }
    );
}

#[test]
fn notifies_connection_point_with_message() {
    let mut point = FakeConnectionPoint::new();
    let point =
        unsafe { Vst3ConnectionPoint::from_raw(point.raw_connection_point()) }.expect("point");
    let mut message = Vst3HostMessage::new();
    let raw_message = message.as_mut_ptr();

    point.notify(&mut message).expect("notify");

    assert_eq!(
        unsafe { fake_connection_point_mut(point.as_mut_ptr()) }.notify_calls,
        1
    );
    assert_eq!(
        unsafe { fake_connection_point_mut(point.as_mut_ptr()) }.last_message,
        raw_message
    );
}

#[test]
fn reports_failed_notify_calls() {
    let mut point = FakeConnectionPoint::new();
    point.notify_result = -20;
    let point =
        unsafe { Vst3ConnectionPoint::from_raw(point.raw_connection_point()) }.expect("point");
    let mut message = Vst3HostMessage::new();

    assert_eq!(
        point.notify(&mut message).expect_err("notify failure"),
        HostError::ConnectionPointCallFailed {
            method: "notify",
            result: -20
        }
    );
}

#[test]
fn rejects_null_notify_message() {
    let mut point = FakeConnectionPoint::new();
    let point =
        unsafe { Vst3ConnectionPoint::from_raw(point.raw_connection_point()) }.expect("point");

    assert_eq!(
        point.notify_raw(ptr::null_mut()).expect_err("null message"),
        HostError::ConnectionPointMessageNull
    );
}

#[repr(C)]
pub(crate) struct FakeConnectionPoint {
    point: IConnectionPoint,
    connect_result: i32,
    disconnect_result: i32,
    notify_result: i32,
    add_ref_calls: u32,
    release_calls: u32,
    connect_calls: u32,
    disconnect_calls: u32,
    notify_calls: u32,
    last_other: *mut IConnectionPoint,
    last_message: *mut IMessage,
}

impl FakeConnectionPoint {
    pub(crate) fn new() -> Self {
        Self {
            point: IConnectionPoint {
                vtable: &FAKE_CONNECTION_POINT_VTABLE,
            },
            connect_result: K_RESULT_OK,
            disconnect_result: K_RESULT_OK,
            notify_result: K_RESULT_OK,
            add_ref_calls: 0,
            release_calls: 0,
            connect_calls: 0,
            disconnect_calls: 0,
            notify_calls: 0,
            last_other: ptr::null_mut(),
            last_message: ptr::null_mut(),
        }
    }

    pub(crate) fn raw_connection_point(&mut self) -> *mut c_void {
        (&mut self.point as *mut IConnectionPoint).cast()
    }
}

static FAKE_CONNECTION_POINT_VTABLE: IConnectionPointVTable = IConnectionPointVTable {
    query_interface: fake_query_interface,
    add_ref: fake_add_ref,
    release: fake_release,
    connect: fake_connect,
    disconnect: fake_disconnect,
    notify: fake_notify,
};

unsafe extern "system" fn fake_query_interface(
    this: *mut IConnectionPoint,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return -1;
    }
    unsafe { *obj = ptr::null_mut() };

    let iid = unsafe { tuid_from_raw(iid) };
    if iid == tuid(VST3_FUNKNOWN_IID) || iid == tuid(VST3_I_CONNECTION_POINT_IID) {
        unsafe { *obj = this.cast() };
        let _ = unsafe { fake_add_ref(this) };
        return K_RESULT_OK;
    }

    1
}

unsafe extern "system" fn fake_add_ref(this: *mut IConnectionPoint) -> u32 {
    let fake = unsafe { fake_connection_point_mut(this) };
    fake.add_ref_calls += 1;
    fake.add_ref_calls
}

unsafe extern "system" fn fake_release(this: *mut IConnectionPoint) -> u32 {
    let fake = unsafe { fake_connection_point_mut(this) };
    fake.release_calls += 1;
    fake.release_calls
}

unsafe extern "system" fn fake_connect(
    this: *mut IConnectionPoint,
    other: *mut IConnectionPoint,
) -> i32 {
    let fake = unsafe { fake_connection_point_mut(this) };
    fake.connect_calls += 1;
    fake.last_other = other;
    fake.connect_result
}

unsafe extern "system" fn fake_disconnect(
    this: *mut IConnectionPoint,
    other: *mut IConnectionPoint,
) -> i32 {
    let fake = unsafe { fake_connection_point_mut(this) };
    fake.disconnect_calls += 1;
    fake.last_other = other;
    fake.disconnect_result
}

unsafe extern "system" fn fake_notify(this: *mut IConnectionPoint, message: *mut IMessage) -> i32 {
    let fake = unsafe { fake_connection_point_mut(this) };
    fake.notify_calls += 1;
    fake.last_message = message;
    fake.notify_result
}

unsafe fn fake_connection_point_mut<'a>(
    this: *mut IConnectionPoint,
) -> &'a mut FakeConnectionPoint {
    unsafe { &mut *this.cast::<FakeConnectionPoint>() }
}

fn tuid(value: &str) -> TUid {
    parse_tuid_hex(value).expect("valid iid")
}

unsafe fn tuid_from_raw(raw: *const i8) -> TUid {
    let mut output = [0; 16];
    if !raw.is_null() {
        unsafe { output.copy_from_slice(std::slice::from_raw_parts(raw.cast::<u8>(), 16)) };
    }
    output
}
