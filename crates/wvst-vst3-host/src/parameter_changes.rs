use std::ffi::c_void;
use std::ptr;

use crate::vst3_abi::{
    IParamValueQueue, IParamValueQueueVTable, IParameterChanges, IParameterChangesVTable,
    K_RESULT_FALSE, K_RESULT_OK, ParamId, ParamValue, TUid, VST3_FUNKNOWN_IID,
    VST3_I_PARAM_VALUE_QUEUE_IID, VST3_I_PARAMETER_CHANGES_IID, parse_tuid_hex,
};
use crate::{HostError, HostResult};

pub const DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vst3ParameterChange {
    pub sample_offset: u16,
    pub parameter_id: ParamId,
    pub value_normalized: ParamValue,
}

#[derive(Debug)]
pub struct Vst3ParameterChanges {
    object: Box<ParameterChangesObject>,
}

impl Vst3ParameterChanges {
    pub fn new(max_changes: usize) -> Self {
        let mut object = Box::new(ParameterChangesObject {
            iface: IParameterChanges {
                vtable: &PARAMETER_CHANGES_VTABLE,
            },
            queues: Vec::with_capacity(max_changes),
            queue_count: 0,
            scratch: Vec::with_capacity(max_changes),
            points: Vec::with_capacity(max_changes),
            max_changes,
        });

        let owner = (&mut *object) as *mut ParameterChangesObject;
        for _ in 0..max_changes {
            object.queues.push(ParamValueQueueObject {
                iface: IParamValueQueue {
                    vtable: &PARAM_VALUE_QUEUE_VTABLE,
                },
                parameter_id: 0,
                point_start: 0,
                point_count: 0,
                owner,
            });
        }

        Self { object }
    }

    pub fn as_raw_ptr(&mut self) -> *mut IParameterChanges {
        &mut self.object.iface
    }

    pub fn clear(&mut self) {
        self.object.clear_active();
    }

    pub fn changes(&self) -> Vec<Vst3ParameterChange> {
        let mut changes = Vec::new();
        self.changes_into(&mut changes);
        changes
    }

    pub fn changes_into(&self, destination: &mut Vec<Vst3ParameterChange>) {
        destination.clear();
        for queue in self.object.active_queues() {
            for point in self.object.queue_points(queue) {
                destination.push(Vst3ParameterChange {
                    sample_offset: point.sample_offset.clamp(0, i32::from(u16::MAX)) as u16,
                    parameter_id: queue.parameter_id,
                    value_normalized: point.value,
                });
            }
        }
    }

    pub fn set_changes(
        &mut self,
        frames: usize,
        changes: &[Vst3ParameterChange],
    ) -> HostResult<()> {
        if changes.len() > self.object.max_changes {
            return Err(HostError::InvalidParameterChangeCount {
                max: self.object.max_changes,
                actual: changes.len(),
            });
        }

        self.object.clear_active();
        self.object.scratch.extend_from_slice(changes);
        self.object.scratch.sort_by(|left, right| {
            left.parameter_id
                .cmp(&right.parameter_id)
                .then(left.sample_offset.cmp(&right.sample_offset))
        });

        let mut current_parameter_id = None;
        let mut queue_index = 0;
        for index in 0..self.object.scratch.len() {
            let change = self.object.scratch[index];
            validate_change(frames, change)?;

            if current_parameter_id != Some(change.parameter_id) {
                if self.object.queue_count >= self.object.queues.len() {
                    return Err(HostError::InvalidParameterChangeCount {
                        max: self.object.queues.len(),
                        actual: self.object.queue_count + 1,
                    });
                }

                queue_index = self.object.queue_count;
                self.object.queue_count += 1;
                let queue = &mut self.object.queues[queue_index];
                queue.parameter_id = change.parameter_id;
                queue.point_start = self.object.points.len();
                queue.point_count = 0;
                current_parameter_id = Some(change.parameter_id);
            }

            self.object.points.push(ParameterPoint {
                sample_offset: i32::from(change.sample_offset),
                value: change.value_normalized,
            });
            self.object.queues[queue_index].point_count += 1;
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Debug)]
struct ParameterChangesObject {
    iface: IParameterChanges,
    queues: Vec<ParamValueQueueObject>,
    queue_count: usize,
    scratch: Vec<Vst3ParameterChange>,
    points: Vec<ParameterPoint>,
    max_changes: usize,
}

impl ParameterChangesObject {
    fn clear_active(&mut self) {
        for index in 0..self.queue_count {
            self.queues[index].point_start = 0;
            self.queues[index].point_count = 0;
        }
        self.queue_count = 0;
        self.scratch.clear();
        self.points.clear();
    }

    fn active_queues(&self) -> &[ParamValueQueueObject] {
        &self.queues[..self.queue_count]
    }

    fn queue_points(&self, queue: &ParamValueQueueObject) -> &[ParameterPoint] {
        &self.points[queue.point_start..queue.point_start + queue.point_count]
    }
}

#[repr(C)]
#[derive(Debug)]
struct ParamValueQueueObject {
    iface: IParamValueQueue,
    parameter_id: ParamId,
    point_start: usize,
    point_count: usize,
    owner: *mut ParameterChangesObject,
}

#[derive(Debug, Clone, Copy)]
struct ParameterPoint {
    sample_offset: i32,
    value: ParamValue,
}

const PARAMETER_CHANGES_VTABLE: IParameterChangesVTable = IParameterChangesVTable {
    query_interface: parameter_changes_query_interface,
    add_ref: parameter_changes_add_ref,
    release: parameter_changes_release,
    get_parameter_count: parameter_changes_get_parameter_count,
    get_parameter_data: parameter_changes_get_parameter_data,
    add_parameter_data: parameter_changes_add_parameter_data,
};

const PARAM_VALUE_QUEUE_VTABLE: IParamValueQueueVTable = IParamValueQueueVTable {
    query_interface: param_value_queue_query_interface,
    add_ref: param_value_queue_add_ref,
    release: param_value_queue_release,
    get_parameter_id: param_value_queue_get_parameter_id,
    get_point_count: param_value_queue_get_point_count,
    get_point: param_value_queue_get_point,
    add_point: param_value_queue_add_point,
};

unsafe extern "system" fn parameter_changes_query_interface(
    this: *mut IParameterChanges,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    query_host_interface(
        this.cast(),
        iid,
        obj,
        VST3_I_PARAMETER_CHANGES_IID,
        ptr::null_mut(),
    )
}

unsafe extern "system" fn parameter_changes_add_ref(_this: *mut IParameterChanges) -> u32 {
    1
}

unsafe extern "system" fn parameter_changes_release(_this: *mut IParameterChanges) -> u32 {
    1
}

unsafe extern "system" fn parameter_changes_get_parameter_count(
    this: *mut IParameterChanges,
) -> i32 {
    parameter_changes_object(this).map_or(0, |object| object.queue_count as i32)
}

unsafe extern "system" fn parameter_changes_get_parameter_data(
    this: *mut IParameterChanges,
    index: i32,
) -> *mut IParamValueQueue {
    if index < 0 {
        return ptr::null_mut();
    }

    let Some(object) = parameter_changes_object_mut(this) else {
        return ptr::null_mut();
    };
    if index as usize >= object.queue_count {
        return ptr::null_mut();
    }

    &mut object.queues[index as usize].iface
}

unsafe extern "system" fn parameter_changes_add_parameter_data(
    this: *mut IParameterChanges,
    id: *const ParamId,
    index: *mut i32,
) -> *mut IParamValueQueue {
    if id.is_null() {
        return ptr::null_mut();
    }

    let Some(object) = parameter_changes_object_mut(this) else {
        return ptr::null_mut();
    };
    if object.queue_count >= object.queues.len() {
        return ptr::null_mut();
    }

    let queue_index = object.queue_count;
    object.queue_count += 1;
    let queue = &mut object.queues[queue_index];
    // SAFETY: `id` was checked for null and points to caller-owned ParamId storage.
    queue.parameter_id = unsafe { *id };
    queue.point_start = object.points.len();
    queue.point_count = 0;
    if !index.is_null() {
        // SAFETY: `index` is an optional out pointer provided by the caller.
        unsafe { *index = queue_index as i32 };
    }

    &mut queue.iface
}

unsafe extern "system" fn param_value_queue_query_interface(
    this: *mut IParamValueQueue,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    query_host_interface(
        this.cast(),
        iid,
        obj,
        VST3_I_PARAM_VALUE_QUEUE_IID,
        ptr::null_mut(),
    )
}

unsafe extern "system" fn param_value_queue_add_ref(_this: *mut IParamValueQueue) -> u32 {
    1
}

unsafe extern "system" fn param_value_queue_release(_this: *mut IParamValueQueue) -> u32 {
    1
}

unsafe extern "system" fn param_value_queue_get_parameter_id(
    this: *mut IParamValueQueue,
) -> ParamId {
    param_value_queue_object(this).map_or(0, |queue| queue.parameter_id)
}

unsafe extern "system" fn param_value_queue_get_point_count(this: *mut IParamValueQueue) -> i32 {
    param_value_queue_object(this).map_or(0, |queue| queue.point_count as i32)
}

unsafe extern "system" fn param_value_queue_get_point(
    this: *mut IParamValueQueue,
    index: i32,
    sample_offset: *mut i32,
    value: *mut ParamValue,
) -> i32 {
    if sample_offset.is_null() || value.is_null() || index < 0 {
        return K_RESULT_FALSE;
    }

    let Some(queue) = param_value_queue_object(this) else {
        return K_RESULT_FALSE;
    };
    let Some(owner) = (unsafe { queue.owner.as_ref() }) else {
        return K_RESULT_FALSE;
    };
    let Some(point) = owner
        .points
        .get(queue.point_start.saturating_add(index as usize))
    else {
        return K_RESULT_FALSE;
    };

    // SAFETY: output pointers were checked for null and receive plain values.
    unsafe {
        *sample_offset = point.sample_offset;
        *value = point.value;
    }
    K_RESULT_OK
}

unsafe extern "system" fn param_value_queue_add_point(
    this: *mut IParamValueQueue,
    sample_offset: i32,
    value: ParamValue,
    index: *mut i32,
) -> i32 {
    if sample_offset < 0 || !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return K_RESULT_FALSE;
    }

    let Some(queue) = param_value_queue_object_mut(this) else {
        return K_RESULT_FALSE;
    };

    let Some(owner) = (unsafe { queue.owner.as_mut() }) else {
        return K_RESULT_FALSE;
    };
    if owner.points.len() >= owner.max_changes {
        return K_RESULT_FALSE;
    }

    let point_index = queue.point_count;
    owner.points.push(ParameterPoint {
        sample_offset,
        value,
    });
    queue.point_count += 1;
    if !index.is_null() {
        // SAFETY: `index` is an optional out pointer provided by the caller.
        unsafe { *index = point_index as i32 };
    }

    K_RESULT_OK
}

fn parameter_changes_object(
    this: *mut IParameterChanges,
) -> Option<&'static ParameterChangesObject> {
    if this.is_null() {
        return None;
    }

    // SAFETY: Vst3ParameterChanges exposes pointers to ParameterChangesObject.iface,
    // which is the first field of the repr(C) object. The object is boxed.
    Some(unsafe { &*this.cast::<ParameterChangesObject>() })
}

fn parameter_changes_object_mut(
    this: *mut IParameterChanges,
) -> Option<&'static mut ParameterChangesObject> {
    if this.is_null() {
        return None;
    }

    // SAFETY: See parameter_changes_object. Mutable access occurs while WVST
    // has exclusive access to the process buffers or through VST3 callbacks.
    Some(unsafe { &mut *this.cast::<ParameterChangesObject>() })
}

fn param_value_queue_object(this: *mut IParamValueQueue) -> Option<&'static ParamValueQueueObject> {
    if this.is_null() {
        return None;
    }

    // SAFETY: IParamValueQueue is the first field of ParamValueQueueObject.
    Some(unsafe { &*this.cast::<ParamValueQueueObject>() })
}

fn param_value_queue_object_mut(
    this: *mut IParamValueQueue,
) -> Option<&'static mut ParamValueQueueObject> {
    if this.is_null() {
        return None;
    }

    // SAFETY: IParamValueQueue is the first field of ParamValueQueueObject.
    Some(unsafe { &mut *this.cast::<ParamValueQueueObject>() })
}

fn validate_change(frames: usize, change: Vst3ParameterChange) -> HostResult<()> {
    if usize::from(change.sample_offset) >= frames {
        return Err(HostError::InvalidEventSampleOffset {
            frames,
            actual: usize::from(change.sample_offset),
        });
    }
    if !change.value_normalized.is_finite() || !(0.0..=1.0).contains(&change.value_normalized) {
        return Err(HostError::InvalidParameterChangeValue {
            parameter_id: change.parameter_id,
            value: change.value_normalized,
        });
    }

    Ok(())
}

fn query_host_interface(
    this: *mut c_void,
    iid: *const i8,
    obj: *mut *mut c_void,
    interface_id: &str,
    fallback: *mut c_void,
) -> i32 {
    if !obj.is_null() {
        // SAFETY: `obj` is an out pointer provided by the caller.
        unsafe { *obj = ptr::null_mut() };
    }
    if iid.is_null() || obj.is_null() {
        return K_RESULT_FALSE;
    }

    let Some(requested_iid) = read_tuid(iid) else {
        return K_RESULT_FALSE;
    };
    if requested_iid == tuid(VST3_FUNKNOWN_IID) || requested_iid == tuid(interface_id) {
        // SAFETY: `obj` was checked for null. The object is host-owned and
        // uses a static lifetime for VST3's non-refcounting process callback.
        unsafe { *obj = if fallback.is_null() { this } else { fallback } };
        K_RESULT_OK
    } else {
        K_RESULT_FALSE
    }
}

fn read_tuid(iid: *const i8) -> Option<TUid> {
    if iid.is_null() {
        return None;
    }

    let mut value = [0; 16];
    // SAFETY: VST3 queryInterface passes a pointer to 16 bytes of TUID data.
    unsafe { ptr::copy_nonoverlapping(iid.cast::<u8>(), value.as_mut_ptr(), 16) };
    Some(value)
}

fn tuid(value: &str) -> TUid {
    parse_tuid_hex(value).expect("built-in VST3 interface id is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_parameter_queues_grouped_by_param_id() {
        let mut changes = Vst3ParameterChanges::new(8);
        changes
            .set_changes(
                64,
                &[
                    Vst3ParameterChange {
                        sample_offset: 10,
                        parameter_id: 2,
                        value_normalized: 0.25,
                    },
                    Vst3ParameterChange {
                        sample_offset: 4,
                        parameter_id: 1,
                        value_normalized: 0.5,
                    },
                    Vst3ParameterChange {
                        sample_offset: 12,
                        parameter_id: 2,
                        value_normalized: 0.75,
                    },
                ],
            )
            .expect("changes");

        let ptr = changes.as_raw_ptr();
        let count = unsafe { ((*(*ptr).vtable).get_parameter_count)(ptr) };
        let queue = unsafe { ((*(*ptr).vtable).get_parameter_data)(ptr, 1) };
        let point_count = unsafe { ((*(*queue).vtable).get_point_count)(queue) };
        let mut sample_offset = 0;
        let mut value = 0.0;
        let result =
            unsafe { ((*(*queue).vtable).get_point)(queue, 1, &mut sample_offset, &mut value) };

        assert_eq!(count, 2);
        assert_eq!(unsafe { ((*(*queue).vtable).get_parameter_id)(queue) }, 2);
        assert_eq!(point_count, 2);
        assert_eq!(result, K_RESULT_OK);
        assert_eq!(sample_offset, 12);
        assert_eq!(value, 0.75);
    }

    #[test]
    fn clears_parameter_changes_between_blocks() {
        let mut changes = Vst3ParameterChanges::new(8);
        changes
            .set_changes(
                64,
                &[Vst3ParameterChange {
                    sample_offset: 0,
                    parameter_id: 1,
                    value_normalized: 0.5,
                }],
            )
            .expect("changes");

        changes.clear();

        let ptr = changes.as_raw_ptr();
        let count = unsafe { ((*(*ptr).vtable).get_parameter_count)(ptr) };

        assert_eq!(count, 0);
    }

    #[test]
    fn rejects_parameter_change_offsets_outside_block() {
        let mut changes = Vst3ParameterChanges::new(8);
        let error = changes
            .set_changes(
                8,
                &[Vst3ParameterChange {
                    sample_offset: 8,
                    parameter_id: 1,
                    value_normalized: 0.5,
                }],
            )
            .expect_err("bad offset");

        assert_eq!(
            error,
            HostError::InvalidEventSampleOffset {
                frames: 8,
                actual: 8
            }
        );
    }

    #[test]
    fn bounds_plugin_output_parameter_points_without_growing() {
        let mut changes = Vst3ParameterChanges::new(1);
        let raw = changes.as_raw_ptr();
        let parameter_id = 7;
        let queue =
            unsafe { ((*(*raw).vtable).add_parameter_data)(raw, &parameter_id, ptr::null_mut()) };

        assert_eq!(
            unsafe { ((*(*queue).vtable).add_point)(queue, 0, 0.5, ptr::null_mut()) },
            K_RESULT_OK
        );
        assert_eq!(
            unsafe { ((*(*queue).vtable).add_point)(queue, 1, 0.75, ptr::null_mut()) },
            K_RESULT_FALSE
        );
        assert_eq!(unsafe { ((*(*queue).vtable).get_point_count)(queue) }, 1);
    }
}
