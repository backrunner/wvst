use std::ptr;

use super::*;
use crate::event_list::{Vst3InputEvent, Vst3NoteEvent};
use crate::parameter_changes::Vst3ParameterChange;
use crate::vst3_abi::IParameterChanges;
use crate::vst3_abi::{Event, IEventList, ParamId, VST3_EVENT_TYPE_NOTE_ON};
use crate::vst3_abi::{ProcessContext, VST3_PROCESS_CONTEXT_PLAYING};

#[test]
fn deinterleaves_stereo_input_to_planar_channels() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 2, 2).expect("buffers");
    let input = [1.0, 10.0, 2.0, 20.0, 3.0, 30.0];

    buffers.prepare_interleaved_f32(3, &input).expect("prepare");

    assert_eq!(
        buffers.input_channel(0, 3),
        Some([1.0, 2.0, 3.0].as_slice())
    );
    assert_eq!(
        buffers.input_channel(1, 3),
        Some([10.0, 20.0, 30.0].as_slice())
    );
    assert_eq!(buffers.process_data.num_samples, 3);
    assert_eq!(buffers.process_data.num_inputs, 1);
    assert_eq!(buffers.process_data.num_outputs, 1);
    assert!(!buffers.process_data.inputs.is_null());
    assert!(!buffers.process_data.outputs.is_null());
}

#[test]
fn copies_planar_output_to_interleaved_buffer() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 2, 2).expect("buffers");
    buffers
        .prepare_interleaved_f32(3, &[0.0; 6])
        .expect("prepare");
    buffers
        .output_channel_mut(0, 3)
        .expect("left")
        .copy_from_slice(&[1.0, 2.0, 3.0]);
    buffers
        .output_channel_mut(1, 3)
        .expect("right")
        .copy_from_slice(&[10.0, 20.0, 30.0]);

    let mut output = [0.0; 6];
    buffers
        .copy_output_to_interleaved(3, &mut output)
        .expect("copy");

    assert_eq!(output, [1.0, 10.0, 2.0, 20.0, 3.0, 30.0]);
}

#[test]
fn clears_each_planar_output_channel_between_blocks() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 2, 2).expect("buffers");
    buffers
        .prepare_interleaved_f32(3, &[0.0; 6])
        .expect("prepare");
    buffers
        .output_channel_mut(0, 3)
        .expect("left")
        .copy_from_slice(&[1.0, 2.0, 3.0]);
    buffers
        .output_channel_mut(1, 3)
        .expect("right")
        .copy_from_slice(&[10.0, 20.0, 30.0]);

    buffers
        .prepare_interleaved_f32(2, &[0.0; 4])
        .expect("prepare smaller block");
    let mut output = [1.0; 4];
    buffers
        .copy_output_to_interleaved(2, &mut output)
        .expect("copy");

    assert_eq!(output, [0.0; 4]);
}

#[test]
fn rejects_frame_count_above_max() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 2, 2, 2).expect("buffers");
    let error = buffers
        .prepare_interleaved_f32(3, &[0.0; 6])
        .expect_err("frame count above max");

    assert_eq!(
        error,
        HostError::InvalidBufferLength {
            expected: 2,
            actual: 3
        }
    );
}

#[test]
fn rejects_invalid_input_and_output_lengths() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 2, 2).expect("buffers");
    let input_error = buffers
        .prepare_interleaved_f32(2, &[0.0; 3])
        .expect_err("input mismatch");
    assert_eq!(
        input_error,
        HostError::InvalidBufferLength {
            expected: 4,
            actual: 3
        }
    );

    buffers
        .prepare_interleaved_f32(2, &[0.0; 4])
        .expect("prepare");
    let mut output = [0.0; 3];
    let output_error = buffers
        .copy_output_to_interleaved(2, &mut output)
        .expect_err("output mismatch");
    assert_eq!(
        output_error,
        HostError::InvalidBufferLength {
            expected: 4,
            actual: 3
        }
    );
}

#[test]
fn supports_instrument_buffers_without_input_bus() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 0, 2).expect("buffers");

    buffers.prepare_interleaved_f32(2, &[]).expect("prepare");
    buffers
        .output_channel_mut(0, 2)
        .expect("left")
        .copy_from_slice(&[0.25, 0.5]);
    buffers
        .output_channel_mut(1, 2)
        .expect("right")
        .copy_from_slice(&[0.75, 1.0]);

    let mut output = [0.0; 4];
    buffers
        .copy_output_to_interleaved(2, &mut output)
        .expect("copy");

    assert_eq!(output, [0.25, 0.75, 0.5, 1.0]);
    assert!(buffers.input_channel(0, 2).is_none());
    assert_eq!(buffers.process_data.num_inputs, 0);
    assert!(buffers.process_data.inputs.is_null());
    assert!(!buffers.process_data.outputs.is_null());
}

#[test]
fn exposes_process_context_and_advances_timeline() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 2, 2).expect("buffers");

    buffers
        .prepare_interleaved_f32(64, &[0.0; 128])
        .expect("first block");
    let context = unsafe {
        &*buffers
            .process_data
            .process_context
            .cast::<ProcessContext>()
    };

    assert!(!buffers.process_data.process_context.is_null());
    assert_eq!(context.sample_rate, 48_000.0);
    assert_eq!(context.project_time_samples, 0);
    assert_eq!(context.continous_time_samples, 64);
    assert_ne!(context.state & VST3_PROCESS_CONTEXT_PLAYING, 0);

    buffers
        .prepare_interleaved_f32(32, &[0.0; 64])
        .expect("second block");
    let context = unsafe {
        &*buffers
            .process_data
            .process_context
            .cast::<ProcessContext>()
    };

    assert_eq!(context.project_time_samples, 64);
    assert_eq!(context.continous_time_samples, 96);
    assert!(context.project_time_music > 0.0);
}

#[test]
fn exposes_prepared_input_events_to_process_data() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 0, 2).expect("buffers");
    buffers.prepare_interleaved_f32(64, &[]).expect("prepare");
    buffers
        .prepare_input_events(
            64,
            &[Vst3InputEvent::NoteOn(Vst3NoteEvent {
                sample_offset: 12,
                channel: 1,
                pitch: 60,
                velocity: 0.75,
                note_id: -1,
            })],
        )
        .expect("events");

    let event_list = buffers.process_data.input_events.cast::<IEventList>();
    let mut event = Event::default();
    let count = unsafe { ((*(*event_list).vtable).get_event_count)(event_list) };
    let result = unsafe { ((*(*event_list).vtable).get_event)(event_list, 0, &mut event) };

    assert!(!event_list.is_null());
    assert_eq!(count, 1);
    assert_eq!(result, 0);
    assert_eq!(event.sample_offset, 12);
    assert_eq!(event.event_type, VST3_EVENT_TYPE_NOTE_ON);
    assert_eq!(unsafe { event.payload.note_on.channel }, 1);
    assert_eq!(unsafe { event.payload.note_on.pitch }, 60);
}

#[test]
fn clears_prepared_input_events_between_blocks() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 0, 2).expect("buffers");
    buffers.prepare_interleaved_f32(64, &[]).expect("prepare");
    buffers
        .prepare_input_events(
            64,
            &[Vst3InputEvent::NoteOn(Vst3NoteEvent {
                sample_offset: 0,
                channel: 0,
                pitch: 60,
                velocity: 1.0,
                note_id: -1,
            })],
        )
        .expect("events");

    buffers
        .prepare_interleaved_f32(64, &[])
        .expect("next block");

    let event_list = buffers.process_data.input_events.cast::<IEventList>();
    let count = unsafe { ((*(*event_list).vtable).get_event_count)(event_list) };

    assert_eq!(count, 0);
}

#[test]
fn exposes_prepared_parameter_changes_to_process_data() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 0, 2).expect("buffers");
    buffers.prepare_interleaved_f32(64, &[]).expect("prepare");
    buffers
        .prepare_input_parameter_changes(
            64,
            &[
                Vst3ParameterChange {
                    sample_offset: 12,
                    parameter_id: 42,
                    value_normalized: 0.25,
                },
                Vst3ParameterChange {
                    sample_offset: 18,
                    parameter_id: 42,
                    value_normalized: 0.75,
                },
            ],
        )
        .expect("parameter changes");

    let changes = buffers
        .process_data
        .input_parameter_changes
        .cast::<IParameterChanges>();
    let count = unsafe { ((*(*changes).vtable).get_parameter_count)(changes) };
    let queue = unsafe { ((*(*changes).vtable).get_parameter_data)(changes, 0) };
    let point_count = unsafe { ((*(*queue).vtable).get_point_count)(queue) };
    let parameter_id = unsafe { ((*(*queue).vtable).get_parameter_id)(queue) };
    let mut sample_offset = 0;
    let mut value = 0.0;
    let result =
        unsafe { ((*(*queue).vtable).get_point)(queue, 1, &mut sample_offset, &mut value) };

    assert!(!changes.is_null());
    assert_eq!(count, 1);
    assert_eq!(parameter_id, 42);
    assert_eq!(point_count, 2);
    assert_eq!(result, 0);
    assert_eq!(sample_offset, 18);
    assert_eq!(value, 0.75);
}

#[test]
fn captures_output_events_written_by_plugin() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 0, 2).expect("buffers");
    buffers.prepare_interleaved_f32(64, &[]).expect("prepare");
    let event_list = buffers.process_data.output_events.cast::<IEventList>();
    let mut event = Event {
        sample_offset: 7,
        event_type: VST3_EVENT_TYPE_NOTE_ON,
        ..Event::default()
    };

    let result = unsafe { ((*(*event_list).vtable).add_event)(event_list, &mut event) };

    assert!(!event_list.is_null());
    assert_eq!(result, 0);
    assert_eq!(buffers.output_events().len(), 1);
    assert_eq!(buffers.output_events()[0].sample_offset, 7);
    assert_eq!(
        buffers.output_events()[0].event_type,
        VST3_EVENT_TYPE_NOTE_ON
    );

    buffers
        .prepare_interleaved_f32(64, &[])
        .expect("next block");
    assert!(buffers.output_events().is_empty());
}

#[test]
fn captures_output_parameter_changes_written_by_plugin() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 0, 2).expect("buffers");
    buffers.prepare_interleaved_f32(64, &[]).expect("prepare");
    let changes = buffers
        .process_data
        .output_parameter_changes
        .cast::<IParameterChanges>();
    let parameter_id: ParamId = 42;
    let mut queue_index = -1;
    let queue = unsafe {
        ((*(*changes).vtable).add_parameter_data)(changes, &parameter_id, &mut queue_index)
    };
    let mut point_index = -1;
    let result = unsafe { ((*(*queue).vtable).add_point)(queue, 9, 0.25, &mut point_index) };

    assert!(!changes.is_null());
    assert!(!queue.is_null());
    assert_eq!(queue_index, 0);
    assert_eq!(result, 0);
    assert_eq!(point_index, 0);
    assert_eq!(
        buffers.output_parameter_changes(),
        vec![Vst3ParameterChange {
            sample_offset: 9,
            parameter_id: 42,
            value_normalized: 0.25,
        }]
    );
    let mut output_changes = Vec::new();
    let stats = buffers.output_parameter_changes_into(&mut output_changes);
    assert_eq!(stats.raw_points, 1);
    assert_eq!(stats.normalized_points, 1);
    assert_eq!(stats.filtered_points, 0);

    buffers
        .prepare_interleaved_f32(64, &[])
        .expect("next block");
    assert!(buffers.output_parameter_changes().is_empty());
}

#[test]
fn reports_filtered_output_parameter_changes_outside_block() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 0, 2).expect("buffers");
    buffers.prepare_interleaved_f32(64, &[]).expect("prepare");
    let changes = buffers
        .process_data
        .output_parameter_changes
        .cast::<IParameterChanges>();
    let parameter_id: ParamId = 42;
    let queue = unsafe {
        ((*(*changes).vtable).add_parameter_data)(changes, &parameter_id, ptr::null_mut())
    };
    let mut point_index = -1;
    let result = unsafe { ((*(*queue).vtable).add_point)(queue, 80, 0.25, &mut point_index) };
    let mut output_changes = Vec::new();

    let stats = buffers.output_parameter_changes_into(&mut output_changes);

    assert_eq!(result, 0);
    assert_eq!(point_index, 0);
    assert!(output_changes.is_empty());
    assert_eq!(stats.raw_points, 1);
    assert_eq!(stats.normalized_points, 0);
    assert_eq!(stats.filtered_points, 1);
}

#[test]
fn clears_prepared_parameter_changes_between_blocks() {
    let mut buffers = Vst3ProcessBuffers::new(48_000.0, 128, 0, 2).expect("buffers");
    buffers.prepare_interleaved_f32(64, &[]).expect("prepare");
    buffers
        .prepare_input_parameter_changes(
            64,
            &[Vst3ParameterChange {
                sample_offset: 0,
                parameter_id: 42,
                value_normalized: 0.5,
            }],
        )
        .expect("parameter changes");

    buffers
        .prepare_interleaved_f32(64, &[])
        .expect("next block");

    let changes = buffers
        .process_data
        .input_parameter_changes
        .cast::<IParameterChanges>();
    let count = unsafe { ((*(*changes).vtable).get_parameter_count)(changes) };

    assert_eq!(count, 0);
}
