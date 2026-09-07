#![cfg(unix)]

use super::*;

fn altered_worker(from: &str, to: &str) -> PathBuf {
    let worker = ready_worker_script();
    let source = std::fs::read_to_string(&worker).expect("fixture");
    assert!(source.contains(from));
    write_framed_worker_script(&worker, &source.replace(from, to));
    worker
}

#[tokio::test]
async fn slow_load_and_state_restore_do_not_relax_heartbeat_deadline() {
    let worker = altered_worker(
        "    if method == \"instance.create\":",
        "    if method in [\"instance.create\", \"instance.setState\", \"worker.metrics\"]:\n        time.sleep(2.5)\n    if method == \"instance.create\":",
    );
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone())
            .with_audio_ipc(false)
            .with_timeout(Duration::from_secs(2))
            .with_load_timeout(Duration::from_secs(8)),
    );
    supervisor
        .start_instance(&record())
        .await
        .expect("slow load");
    supervisor
        .set_state(1, None, None)
        .await
        .expect("slow restore");
    let error = supervisor
        .heartbeat_instance(1)
        .await
        .expect_err("heartbeat deadline");
    assert!(matches!(
        error,
        WorkerSupervisorError::Timeout {
            method: "worker.metrics",
            timeout_ms: 2000,
            ..
        }
    ));
    assert!(supervisor.processes.lock().await.is_empty());
    let _ = std::fs::remove_dir_all(worker.parent().unwrap());
}

#[tokio::test]
async fn hung_load_is_terminated_and_releases_instance_slot() {
    let worker = altered_worker(
        "    if method == \"instance.create\":",
        "    if method == \"instance.create\":\n        time.sleep(10)",
    );
    let metrics = Arc::new(BridgeMetrics::new());
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone())
            .with_audio_ipc(false)
            .with_load_timeout(Duration::from_millis(50))
            .with_max_instances(1)
            .with_metrics(Arc::clone(&metrics)),
    );
    let error = supervisor
        .start_instance(&record())
        .await
        .expect_err("load deadline");
    assert!(matches!(
        error,
        WorkerSupervisorError::Timeout {
            method: "instance.create",
            timeout_ms: 50,
            ..
        }
    ));
    assert_eq!(supervisor.instance_slots.available_permits(), 1);
    assert!(supervisor.processes.lock().await.is_empty());
    assert_eq!(metrics.snapshot().worker_shutdowns, 1);
    assert_eq!(metrics.snapshot().worker_wait_timeouts, 0);
    let _ = std::fs::remove_dir_all(worker.parent().unwrap());
}

#[tokio::test]
async fn concurrently_loading_workers_obey_instance_limit() {
    let worker = altered_worker(
        "    if method == \"instance.create\":",
        "    if method == \"instance.create\":\n        time.sleep(0.2)",
    );
    let supervisor = Arc::new(WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone())
            .with_audio_ipc(false)
            .with_max_instances(1),
    ));
    let starting = Arc::clone(&supervisor);
    let task = tokio::spawn(async move { starting.start_instance(&record()).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while supervisor.instance_slots.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("slot reserved before loading");
    let error = supervisor
        .start_instance(&InstanceRecord {
            instance_id: 2,
            ..record()
        })
        .await
        .expect_err("loading counts");
    assert!(matches!(
        error,
        WorkerSupervisorError::ResourceLimitExceeded {
            active: 1,
            limit: 1
        }
    ));
    task.await.expect("task").expect("first load completes");
    supervisor.destroy_instance(1).await.expect("destroy");
    assert_eq!(supervisor.instance_slots.available_permits(), 1);
    let _ = std::fs::remove_dir_all(worker.parent().unwrap());
}

#[tokio::test]
async fn successful_reloads_do_not_erase_repeated_runtime_crashes() {
    let worker = altered_worker(
        "    if method == \"worker.metrics\":\n        return ok(request, runtime_metrics())",
        "    if method == \"worker.metrics\":\n        os._exit(9)",
    );
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone()).with_audio_ipc(false),
    );
    for _ in 0..DEFAULT_QUARANTINE_FAILURE_THRESHOLD {
        supervisor
            .start_instance(&record())
            .await
            .expect("loads successfully");
        assert!(supervisor.heartbeat_instance(1).await.is_err());
        // A subsequent request for the already removed process cannot count twice.
        assert!(supervisor.heartbeat_instance(1).await.is_err());
    }
    assert_eq!(
        supervisor.quarantine_failures(&record().plugin_id).await,
        Some(3)
    );
    assert!(matches!(
        supervisor.start_instance(&record()).await,
        Err(WorkerSupervisorError::Quarantined { failures: 3, .. })
    ));
    let _ = std::fs::remove_dir_all(worker.parent().unwrap());
}

#[tokio::test]
async fn stderr_tail_is_bounded_even_without_newlines() {
    let worker = altered_worker(
        "    if method == \"instance.create\":",
        "    if method == \"instance.create\":\n        sys.stderr.write('x' * 1000000 + 'tail-marker')\n        sys.stderr.flush()",
    );
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone()).with_audio_ipc(false),
    );
    supervisor
        .start_instance(&record())
        .await
        .expect("loads despite stderr flood");
    let process = supervisor.processes.lock().await[&1].clone();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let tail = process.lock().await.stderr.snapshot().await;
            assert!(tail.len() <= STDERR_TAIL_BYTES);
            if tail.ends_with("tail-marker") {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("partial line captured without waiting for newline");
    supervisor.destroy_instance(1).await.expect("destroy");
    let _ = std::fs::remove_dir_all(worker.parent().unwrap());
}

#[tokio::test]
async fn rejected_optional_queries_do_not_kill_or_quarantine_a_healthy_worker() {
    let worker = altered_worker(
        "    if method == \"instance.units\":\n        return ok(request, {\"instanceId\": 1, \"unitInfo\": None})",
        "    if method == \"instance.units\":\n        return err(request, 4220, \"optional interface unavailable\", {\"kind\": \"vst3-runtime-control\"})",
    );
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone()).with_audio_ipc(false),
    );
    supervisor.start_instance(&record()).await.expect("start");
    for _ in 0..4 {
        assert!(supervisor.units(1).await.is_err());
        assert!(supervisor.metadata_refresh(1, false, true).await.is_err());
        supervisor
            .heartbeat_instance(1)
            .await
            .expect("still healthy");
    }
    assert!(
        supervisor
            .quarantine_status(&record().plugin_id)
            .await
            .is_none()
    );
    supervisor.destroy_instance(1).await.expect("destroy");
    let _ = std::fs::remove_dir_all(worker.parent().unwrap());
}
