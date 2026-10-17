use super::{Refusal, lower_with, run, run_with_policy, write_warning};
use std::cell::RefCell;

#[test]
fn priority_is_applied_before_any_helper_startup_and_preserves_its_result() {
    let events = RefCell::new(Vec::new());
    let result = run_with_policy(
        || {
            events.borrow_mut().push("helper threads and graphics");
            37
        },
        || {
            events.borrow_mut().push("child priority");
            Ok(())
        },
        |_| panic!("successful preference must not warn"),
    );
    assert_eq!(result, 37);
    assert_eq!(
        *events.borrow(),
        ["child priority", "helper threads and graphics"]
    );
}

#[test]
fn every_optional_policy_refusal_warns_before_continuing_original_helper_work() {
    for refusal in [
        Refusal::UnknownInherited,
        Refusal::Lowering,
        Refusal::Verification,
    ] {
        let events = RefCell::new(Vec::new());
        let result = run_with_policy(
            || {
                events.borrow_mut().push("original helper work".to_owned());
                11
            },
            || Err(refusal),
            |warning| {
                assert!(warning.starts_with("embedded Stage CPU priority:"));
                assert!(warning.contains("continuing"));
                assert!(warning.contains("check launcher scheduling restrictions"));
                events.borrow_mut().push(warning.to_owned());
            },
        );
        assert_eq!(result, 11);
        assert_eq!(events.borrow().len(), 2);
        assert_eq!(events.borrow()[1], "original helper work");
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn non_linux_embedded_startup_keeps_original_calling_thread_and_result() {
    let original = std::thread::current().id();
    assert_eq!(
        run(|| {
            assert_eq!(std::thread::current().id(), original);
            19
        }),
        19
    );
}

#[cfg(target_os = "linux")]
fn current_nice() -> i32 {
    rustix::process::getpriority_process(None).expect("read actual calling thread nice")
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "private subprocess probe invoked by linux_children_threads_and_replacements_leave_parent_unchanged"]
fn linux_child_probe() {
    let increment: i32 = std::env::var("LIGHT_EMBEDDED_PRIORITY_TEST_INCREMENT")
        .expect("owned probe input")
        .parse()
        .expect("numeric owned probe input");
    assert!((0..=19).contains(&increment));
    rustix::process::nice(increment).expect("positive setup increment only in owned child");
    let inherited = current_nice();
    let expected = (inherited + 5).min(19);
    let result = run(|| {
        assert_eq!(
            current_nice(),
            expected,
            "actual startup policy precedes helper threads"
        );
        let workers = (0..4)
            .map(|_| std::thread::spawn(current_nice))
            .collect::<Vec<_>>();
        for worker in workers {
            assert_eq!(
                worker.join().expect("owned helper thread"),
                expected,
                "future helper threads inherit demoted priority before any graphics startup"
            );
        }
        23
    });
    assert_eq!(result, 23);
    assert_eq!(current_nice(), expected);
    println!(
        "embedded-priority-probe:{}",
        serde_json::json!({"inherited":inherited,"actual":current_nice(),"workers":4})
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_children_threads_and_replacements_leave_parent_unchanged() {
    let parent = current_nice();
    let probe_name = format!(
        "{}::linux_child_probe",
        module_path!()
            .split_once("::")
            .expect("probe module path")
            .1
    );
    for increment in [0, 12, 18, 19] {
        for _replacement in 0..2 {
            let child =
                std::process::Command::new(std::env::current_exe().expect("test executable"))
                    .args(["--exact", &probe_name, "--ignored", "--nocapture"])
                    .env(
                        "LIGHT_EMBEDDED_PRIORITY_TEST_INCREMENT",
                        increment.to_string(),
                    )
                    .output()
                    .expect("start owned no-GPU child probe");
            assert!(
                child.status.success(),
                "child probe failed: {}",
                String::from_utf8_lossy(&child.stderr)
            );
            let text = String::from_utf8(child.stdout).expect("probe stdout");
            let line = text
                .lines()
                .find_map(|line| line.strip_prefix("embedded-priority-probe:"))
                .expect("actual child proof, not an empty filtered test run");
            let proof: serde_json::Value = serde_json::from_str(line).expect("bounded probe JSON");
            assert_eq!(proof["inherited"], (parent + increment).min(19));
            assert_eq!(proof["actual"], (parent + increment + 5).min(19));
            assert_eq!(proof["workers"], 4);
            assert_eq!(
                current_nice(),
                parent,
                "never renice the desktop/headless launching context"
            );
        }
    }
    assert_eq!(
        std::thread::spawn(current_nice)
            .join()
            .expect("ordinary parent thread"),
        parent
    );
}

#[test]
fn positive_increment_never_raises_priority_and_unknown_inheritance_never_calls_os() {
    for inherited in -20..=19 {
        assert_eq!(
            lower_with(
                || Ok(inherited),
                |increment| {
                    assert_eq!(increment, 5);
                    Ok(())
                },
                || Ok((inherited + 5).min(19))
            ),
            Ok(())
        );
    }
    for inherited in [Err(()), Ok(-21), Ok(20)] {
        assert_eq!(
            lower_with(
                || inherited,
                |_| panic!("unknown inheritance must not change priority"),
                || panic!("unknown inheritance must not reach verification")
            ),
            Err(Refusal::UnknownInherited)
        );
    }
    assert_eq!(
        lower_with(
            || Ok(0),
            |_| Err(()),
            || panic!("refused change has no verification")
        ),
        Err(Refusal::Lowering)
    );
    assert_eq!(
        lower_with(|| Ok(10), |_| Ok(()), || Ok(5)),
        Err(Refusal::Verification)
    );
    assert_eq!(
        lower_with(|| Ok(10), |_| Ok(()), || Err(())),
        Err(Refusal::Verification)
    );
}

#[test]
fn an_unwritable_private_warning_cannot_abort_renderer_startup() {
    struct ClosedStderr;
    impl std::io::Write for ClosedStderr {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }
    assert_eq!(
        run_with_policy(
            || 29,
            || Err(Refusal::Lowering),
            |warning| {
                write_warning(&mut ClosedStderr, warning);
            }
        ),
        29
    );
}
