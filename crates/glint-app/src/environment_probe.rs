//! No screenshot files are created by this explicit resource-lifetime probe.
use std::time::Instant;

pub fn run(iterations: usize) -> Result<(), Box<dyn std::error::Error>> {
    if !(1..=1000).contains(&iterations) {
        return Err("Environment probe iterations must be 1..1000".into());
    }
    let environment = glint_platform::desktop_environment()?;
    eprintln!("M0 environment: {environment:?}");
    if environment.dpi_awareness != 2 {
        return Err(
            "Expected per-monitor DPI awareness after initializing the Slint backend".into(),
        );
    }
    // One warm-up excludes lazy process/driver initialization from the release check.
    drop(glint_platform::capture_desktop()?);
    let baseline = glint_platform::gui_resource_counts()?;
    let mut peak = baseline;
    let mut timings = Vec::with_capacity(iterations);
    let mut capture_size = (0, 0);
    let mut capture_origin = (0, 0);
    let left = environment
        .displays
        .iter()
        .map(|d| d.bounds.left)
        .min()
        .unwrap();
    let top = environment
        .displays
        .iter()
        .map(|d| d.bounds.top)
        .min()
        .unwrap();
    let right = environment
        .displays
        .iter()
        .map(|d| d.bounds.right)
        .max()
        .unwrap();
    let bottom = environment
        .displays
        .iter()
        .map(|d| d.bounds.bottom)
        .max()
        .unwrap();
    for _ in 0..iterations {
        if glint_platform::desktop_environment()? != environment {
            return Err(
                "Display topology changed during capture probe; repeat in the new environment"
                    .into(),
            );
        }
        let begin = Instant::now();
        let frame = glint_platform::capture_desktop()?;
        capture_size = (frame.width, frame.height);
        capture_origin = (frame.x, frame.y);
        if capture_origin != (left, top)
            || i64::from(frame.width) != i64::from(right) - i64::from(left)
            || i64::from(frame.height) != i64::from(bottom) - i64::from(top)
        {
            return Err("GDI capture bounds differ from enumerated physical display bounds".into());
        }
        // Force the captured allocation to be released before sampling native resource counts.
        drop(frame);
        timings.push(begin.elapsed().as_secs_f64() * 1000.0);
        let counts = glint_platform::gui_resource_counts()?;
        peak = (peak.0.max(counts.0), peak.1.max(counts.1));
        if counts != baseline {
            return Err(format!("GUI resource counts changed after capture: baseline {baseline:?}, observed {counts:?}").into());
        }
    }
    if glint_platform::desktop_environment()? != environment {
        return Err("Display topology changed before the probe completed".into());
    }
    timings.sort_by(f64::total_cmp);
    println!("{{\"probe\":\"m0-environment\",\"iterations\":{iterations},\"dpi_awareness\":{},\"remote_session\":{},\"displays\":[{}],\"capture_origin\":[{},{}],\"capture_size\":[{},{}],\"gdi_baseline\":{},\"gdi_peak_after_release\":{},\"user_baseline\":{},\"user_peak_after_release\":{},\"capture_and_release_p50_ms\":{},\"capture_and_release_p95_ms\":{},\"capture_and_release_max_ms\":{}}}",
        environment.dpi_awareness, environment.remote_session,
        environment.displays.iter().map(|display| format!("{{\"bounds\":[{},{},{},{}],\"work_area\":[{},{},{},{}],\"primary\":{},\"effective_dpi\":[{},{}]}}", display.bounds.left, display.bounds.top, display.bounds.right, display.bounds.bottom, display.work_area.left, display.work_area.top, display.work_area.right, display.work_area.bottom, display.primary, display.effective_dpi.0, display.effective_dpi.1)).collect::<Vec<_>>().join(","),
        capture_origin.0, capture_origin.1, capture_size.0, capture_size.1, baseline.0, peak.0, baseline.1, peak.1,
        timings[iterations / 2], timings[((iterations as f64 * 0.95).ceil() as usize).saturating_sub(1)], timings[iterations - 1]);
    Ok(())
}
