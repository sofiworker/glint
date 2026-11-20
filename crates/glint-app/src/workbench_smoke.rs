//! Native adapter/software-renderer regression, independent of resident settings.
use crate::workbench::*;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

pub(crate) fn active_window() -> Option<WorkbenchWindow> {
    CURRENT.with_borrow(|s| s.as_ref().map(|s| s.borrow().ui.clone_strong()))
}

pub fn run(directory: &Path, interactive: bool) -> Result<(), Box<dyn std::error::Error>> {
    if directory.exists() && std::fs::read_dir(directory)?.next().is_some() {
        return Err("Use a new, empty workbench QA directory".into());
    }
    std::fs::create_dir_all(directory)?;
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .with_winit_window_attributes_hook(crate::motion::window_attributes)
        .select()?;
    let mut pixels = image::RgbaImage::from_pixel(960, 600, image::Rgba([250, 251, 253, 255]));
    for (y, row) in pixels.rows_mut().enumerate() {
        for (x, p) in row.enumerate() {
            if x > 80 && x < 880 && y > 72 && y < 126 {
                *p = image::Rgba([75, 92, 250, 255]);
            } else if x > 80 && x < 750 && (200..400).contains(&y) && y % 64 < 14 {
                *p = image::Rgba([210, 216, 232, 255]);
            }
        }
    }
    let doc = glint_core::ImageDocument::from_selection(
        pixels.as_raw(),
        960,
        600,
        glint_core::Rect {
            x: 0,
            y: 0,
            width: 960,
            height: 600,
        },
        &glint_core::Edits::default(),
    )?;
    let presentation = Presentation {
        english: false,
        dark: false,
        theme: glint_core::config::Theme::Light,
        contrast: false,
        accent: slint::Color::from_rgb_u8(75, 92, 250),
        scale: glint_core::config::Scale::Percent100,
        reduced: true,
        output: glint_core::config::Output::default(),
    };
    open(Document::capture(Arc::new(doc)), presentation.clone())?;
    if interactive {
        return slint::run_event_loop().map_err(Into::into);
    }
    let output = directory.to_owned();
    let done: Rc<RefCell<Result<(), String>>> = Rc::new(RefCell::new(Ok(())));
    let result = done.clone();
    let started = Instant::now();
    let mut phase = 0;
    let mut hint_index = 0usize;
    let mut hint_started: Option<Instant> = None;
    let mut hint_before: Option<image::RgbaImage> = None;
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(60),move||{
        let check=(|| -> Result<bool,String>{
            if started.elapsed()>Duration::from_secs(60){return Err(format!("Workbench QA timeout in phase {phase}, tooltip {hint_index}"));}
            let Some(session)=CURRENT.with_borrow(Clone::clone)else{return Ok(false);};
            let ui=session.borrow().ui.clone_strong();
            if ui.get_busy(){return Ok(false);}
            if ui.get_picture().size().width==0{return Ok(false);}
            if ui.get_render_ticket()!=ui.get_rendered_ticket(){return Ok(false);}
            match phase {
                0=>{
                    snapshot(&ui,&output.join("image-light.png"))?;
                    let size=ui.window().size().to_logical(ui.window().scale_factor());
                    ui.invoke_action("tool:1".into());
                    pointer(&ui,0,size.width/2.-100.,size.height/2.);
                    pointer(&ui,1,size.width/2.+100.,size.height/2.+80.);
                    pointer(&ui,2,size.width/2.+100.,size.height/2.+80.);
                    assert_eq!(session.borrow().model.state.marks.len(),1);assert!(ui.get_dirty());
                    click(&ui,28.,71.);assert!(session.borrow().model.state.marks.is_empty(),"Pointer click must reach Undo through the focus scope");assert!(!ui.get_dirty());
                    click(&ui,64.,71.);assert_eq!(session.borrow().model.state.marks.len(),1,"Pointer click must reach Redo");
                    let before_menu=session.borrow().model.state.clone();
                    click(&ui,size.width-28.,71.);assert!(ui.get_more(),"Pointer click must open More");
                    key(&ui,slint::platform::Key::Escape);assert!(!ui.get_more());assert_eq!(session.borrow().model.state,before_menu);
                    click(&ui,size.width-28.,71.);assert!(ui.get_more());
                    click(&ui,100.,200.);assert!(!ui.get_more());assert_eq!(session.borrow().model.state,before_menu,"Outside click must only dismiss the menu");
                    let before=session.borrow().model.state.clone();
                    ui.invoke_action("category:0".into());ui.invoke_action("rotate".into());ui.invoke_action("flip".into());ui.invoke_action("crop-cancel".into());assert_eq!(session.borrow().model.state,before);
                    ui.invoke_action("category:0".into());ui.invoke_action("rotate".into());ui.invoke_action("crop-apply".into());assert_ne!(session.borrow().model.state,before);ui.invoke_action("undo".into());assert_eq!(session.borrow().model.state,before);
                    ui.invoke_action("category:2".into());ui.invoke_pointer(0,180.,340.,1.);ui.set_text_value("拾光 · 编辑文字".into());ui.invoke_action("text-commit".into());
                }
                1=>{
                    assert_eq!(session.borrow().model.state.marks.len(),2);assert!(!ui.get_text_editing());
                    ui.invoke_edit_text();assert!(ui.get_text_editing());ui.set_text_value("已修改".into());
                    ui.invoke_action("close".into());assert!(ui.get_text_editing());assert_eq!(ui.get_text_value(),"已修改");assert_eq!(ui.get_modal(),0);
                    ui.invoke_action("text-cancel".into());
                    assert_eq!(session.borrow().model.state.marks[1].annotation.as_ref().bounds().x,180);
                    ui.invoke_value("compare".into(),1.);assert!(ui.get_comparing());ui.invoke_value("compare".into(),0.);
                    ui.invoke_action("close".into());assert_eq!(ui.get_modal(),1);
                }
                2=>{
                    assert!(ui.get_modal_cancel_focused());
                    snapshot(&ui,&output.join("unsaved-dialog.png"))?;
                    let before=session.borrow().model.state.clone();
                    for _ in 0..3 { key(&ui,slint::platform::Key::Tab); }
                    assert!(ui.get_modal_cancel_focused(),"Tab must stay within the unsaved dialog");
                    assert_eq!(session.borrow().model.state,before);
                    key(&ui,slint::platform::Key::Return);assert_eq!(ui.get_modal(),0);assert!(session.borrow().leave.is_none());
                    let owner=crate::motion::create_settings(crate::settings::SettingsWindow::new).map_err(|e|e.to_string())?;
                    assert!(request_quit(&owner));assert!(matches!(session.borrow().leave,Some(Leave::Quit(_))));
                    ui.invoke_action("modal-cancel".into());assert!(session.borrow().leave.is_none());
                    session.borrow().cancel.store(false,Ordering::Release);ui.set_busy(true);ui.invoke_focus_canvas();
                    key(&ui,slint::platform::Key::Escape);assert!(session.borrow().cancel.load(Ordering::Acquire),"Esc must reach the running export");
                    ui.set_busy(false);session.borrow().cancel.store(false,Ordering::Release);
                    ui.invoke_action("category:1".into());
                    snapshot(&ui,&output.join("image-edited.png"))?;
                    ui.invoke_action("save".into());assert_eq!(ui.get_modal(),4);
                    ui.set_export_format(2);ui.set_export_size(2);
                    let selected=export::selection(&session.borrow(),false)?;
                    assert_eq!(selected.extension,"webp");assert_eq!(selected.state.transform.dimensions(),(480,300));assert_eq!(session.borrow().model.state.transform.dimensions(),(960,600));
                    assert_eq!(export::selection(&session.borrow(),true)?.state.transform.dimensions(),(960,600));
                    snapshot(&ui,&output.join("image-export.png"))?;
                    ui.set_export_format(1);ui.set_export_jpeg_quality(85);
                    snapshot(&ui,&output.join("jpeg-export.png"))?;
                    ui.invoke_action("modal-cancel".into());
                    ui.set_dark(true);
                    ui.window().set_size(slint::LogicalSize::new(740.,540.));
                }
                3=>{
                    snapshot(&ui,&output.join("image-dark-small.png"))?;
                    let gif_path=output.join("fixture.gif");
                    let mut gif=image::codecs::gif::GifEncoder::new(std::fs::File::create(&gif_path).map_err(|e|e.to_string())?);
                    for color in [[70,120,220,255],[200,90,120,255],[40,170,130,255]] {gif.encode_frame(image::Frame::from_parts(image::RgbaImage::from_pixel(320,200,image::Rgba(color)),0,0,image::Delay::from_numer_denom_ms(1000,1))).map_err(|e|e.to_string())?;}
                    drop(gif);let document=decode(&gif_path)?;
                    create(document,presentation.clone())?;
                }
                4=>{
                    assert!(ui.get_video());assert_eq!(session.borrow().model.state.clips.len(),1);
                    ui.invoke_value("seek".into(),1.2);ui.invoke_action("split".into());assert_eq!(session.borrow().model.state.clips.len(),2);
                    ui.invoke_action("clip:0".into());ui.invoke_action("delete-clip".into());assert_eq!(session.borrow().model.state.clips.len(),1);assert!((session.borrow().model.state.duration()-1.8).abs()<0.01);
                    ui.invoke_action("undo".into());ui.invoke_action("frame-next".into());
                }
                5=>{
                    snapshot(&ui,&output.join("video.png"))?;
                    ui.invoke_action("play".into());assert!(ui.get_playing());
                }
                6=>{
                    assert!(ui.get_position()>0.);ui.invoke_action("play".into());assert!(!ui.get_playing());
                    let pending=crate::result_float::prepare_document(session.borrow().doc.clone())?;
                    crate::result_float::show(pending,presentation.clone(),true)?;
                }
                7=>{
                    crate::result_float::smoke_snapshot(&output.join("thumbnail.png"))?;
                    ui.invoke_action("save".into());assert_eq!(ui.get_modal(),4);
                }
                8=>{
                    if ui.get_export_codec_ids().row_count()==0 {return Ok(false);}
                    snapshot(&ui,&output.join("mp4-export.png"))?;
                    ui.set_export_size(2);ui.set_export_fps(4);ui.set_export_audio(3);
                    let selected=export::selection(&session.borrow(),false)?;
                    assert_eq!(selected.state.transform.dimensions(),(160,100));assert_eq!(selected.video.fps,Some(15));assert_eq!(selected.video.audio_bitrate,None);
                    std::fs::write(output.join("codecs.txt"),format!("{:#?}",glint_platform::recording::export_codecs())).map_err(|e|e.to_string())?;
                    assert_eq!(selected.video.audio_bitrate,None,"A silent source must remain silent");
                    ui.window().set_size(slint::LogicalSize::new(740.,540.));
                }
                9=>{
                    snapshot(&ui,&output.join("mp4-export-small.png"))?;
                    ui.set_export_format(1);
                }
                10=>{
                    assert_eq!(ui.get_export_fps(),0);ui.set_export_fps(3);
                    let selected=export::selection(&session.borrow(),false)?;
                    assert_eq!(selected.video.fps,Some(15));assert_eq!(selected.video.audio_bitrate,None);
                    snapshot(&ui,&output.join("gif-export-small.png"))?;
                    ui.set_export_format(2);
                }
                11=>{
                    if ui.get_mov_codec_ids().row_count()>0 {
                        let selected=export::selection(&session.borrow(),false)?;
                        assert_eq!(selected.extension,"mov");assert_eq!(selected.video.container,glint_core::media_export::VideoContainer::Mov);
                    }else{assert!(export::selection(&session.borrow(),false).is_err());}
                    snapshot(&ui,&output.join("mov-export-small.png"))?;
                    ui.set_export_format(3);
                }
                12=>{
                    if ui.get_mkv_codec_ids().row_count()>0 {let selected=export::selection(&session.borrow(),false)?;assert_eq!(selected.extension,"mkv");}else{assert!(export::selection(&session.borrow(),false).is_err());}
                    snapshot(&ui,&output.join("mkv-export-small.png"))?;
                    ui.set_export_format(4);
                }
                13=>{
                    if ui.get_webm_available() {let selected=export::selection(&session.borrow(),false)?;assert_eq!(selected.extension,"webm");assert_eq!(selected.video.codec,glint_core::media_export::VideoCodec::Vp9);}else{assert!(export::selection(&session.borrow(),false).is_err());}
                    snapshot(&ui,&output.join("webm-export-small.png"))?;
                    // Check the failure state without changing system codec registrations or files.
                    ui.set_webm_available(false);
                }
                14=>{
                    assert!(export::selection(&session.borrow(),false).is_err());
                    snapshot(&ui,&output.join("format-unavailable-small.png"))?;
                    ui.invoke_action("modal-cancel".into());
                }
                15..=35=>{
                    let case=((phase-15)/3) as usize;
                    let (english,width,height,scale)=LAYOUT_CASES[case];
                    match (phase-15)%3 {
                        0=>{
                            // Create each scale with the normal presentation path. Directly
                            // dispatching DPI then snapshotting before the OS resize leaves
                            // winit's pixel buffer and the software renderer at different sizes.
                            let mut policy=presentation.clone();
                            policy.english=english;
                            policy.dark=english;
                            policy.theme=if english { glint_core::config::Theme::Dark } else { glint_core::config::Theme::Light };
                            policy.scale=scale;
                            let document=session.borrow().doc.clone();
                            create(document,policy)?;
                        }
                        1=>{
                            // Winit converts LogicalSize with the monitor DPI, while
                            // Glint can use an independent UI scale. Request pixels
                            // explicitly so this case really has the intended logical size.
                            let factor=crate::interface_scale::factor(scale);
                            ui.window().set_size(slint::PhysicalSize::new((width*factor).round() as u32,(height*factor).round() as u32));
                            ui.invoke_action("category:2".into());
                            ui.invoke_pointer(0,40.,50.,1.);
                            ui.set_text_value("Layout".into());
                            ui.invoke_action("text-commit".into());
                        }
                        _=>{
                            assert!(ui.get_selected(),"The video annotation must remain selected");
                            let actual=ui.window().size().to_logical(ui.window().scale_factor());
                            if (actual.width-width).abs()>1. || (actual.height-height).abs()>1. {return Ok(false);}
                            let expected_scale=crate::interface_scale::factor(scale);
                            if (ui.window().scale_factor()-expected_scale).abs()>0.01 {return Ok(false);}
                            ui.set_mark_from(12345.6);
                            ui.set_mark_to(86400.);
                            ui.set_time_label("01:23:45.6 / 24:00:00.0".into());
                            let filename=format!("video-selected-{}-{}-scale-{}.png",if english {"en"} else {"zh"},width as u32,(expected_scale*100.) as u32);
                            snapshot(&ui,&output.join(filename))?;
                            assert!(ui.get_timing_content_fits(),"Video timing text clipped in layout case {case}");
                            assert!(ui.get_video_layout_fits(),"Video playback/labels clipped in layout case {case}");
                        }
                    }
                }
                36|39=>{
                    let mut policy=presentation.clone();
                    policy.english=phase==39;
                    policy.dark=phase==39;
                    policy.theme=if phase==39 { glint_core::config::Theme::Dark } else { glint_core::config::Theme::Light };
                    let document=session.borrow().doc.clone();
                    create(document,policy)?;
                    hint_index=0;hint_started=None;hint_before=None;
                }
                37|40=>{
                    ui.window().set_size(slint::PhysicalSize::new(if phase==37 {1100} else {740},if phase==37 {780} else {540}));
                    ui.invoke_action("category:5".into());
                }
                38|41=>{
                    let probes=ui.get_tooltip_probes();
                    if let Some(probe)=probes.row_data(hint_index) {
                        if hint_started.is_none() {
                            pointer(&ui,1,0.,0.);
                            hint_before=Some(window_pixels(&ui)?);
                            pointer(&ui,1,probe.x,probe.y);
                            hint_started=Some(Instant::now());
                            return Ok(false);
                        }
                        if hint_started.unwrap().elapsed()<Duration::from_millis(650) {return Ok(false);}
                        let pixels=window_pixels(&ui)?;
                        pixels.save(output.join(format!("tooltip-{}-{hint_index:02}.png",if phase==38 {"zh"} else {"en"}))).map_err(|e|e.to_string())?;
                        verify_tooltip(hint_before.as_ref().unwrap(),&pixels,&probe,ui.window().scale_factor()).map_err(|e|format!("Tooltip {hint_index}, phase {phase}: {e}"))?;
                        hint_index+=1;hint_started=None;hint_before=None;
                        return Ok(false);
                    }
                    // A visible tooltip must not intercept the button's click.
                    let more=probes.row_data(13).unwrap();
                    click(&ui,more.x,more.y);assert!(ui.get_more());
                    key(&ui,slint::platform::Key::Escape);assert!(!ui.get_more());
                    if phase==41 {
                        std::fs::write(output.join("result.txt"),"PASS: native image/video adapters and export panels; selected annotation timing/layout in Chinese/English, 740x540 and 1100x780, actual 100/125/150/175 percent rendering; rendered tooltip text bounds above property rows/canvas/timeline for all 20 toolbar, playback and zoom icons in Chinese/light and English/dark, right-edge containment and pointer click-through. This does not verify physical input, mixed DPI or long recordings.").map_err(|e|e.to_string())?;
                        return Ok(true);
                    }
                }
                _=>return Ok(true),
            }
            phase+=1;Ok(false)
        })();
        match check {Ok(false)=>{},Ok(true)=>{shutdown();let _=slint::quit_event_loop();},Err(e)=>{*result.borrow_mut()=Err(e);shutdown();let _=slint::quit_event_loop();}}
    });
    slint::run_event_loop()?;
    done.borrow().clone().map_err(Into::into)
}
const LAYOUT_CASES: [(bool, f32, f32, glint_core::config::Scale); 7] = {
    use glint_core::config::Scale::*;
    [
        (false, 740., 540., Percent100),
        (false, 1100., 780., Percent100),
        (true, 740., 540., Percent100),
        (true, 1100., 780., Percent100),
        (true, 740., 540., Percent125),
        (true, 740., 540., Percent150),
        (true, 740., 540., Percent175),
    ]
};
fn window_pixels(ui: &WorkbenchWindow) -> Result<image::RgbaImage, String> {
    let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    image::RgbaImage::from_raw(pixels.width(), pixels.height(), pixels.as_bytes().to_vec())
        .ok_or_else(|| "Invalid window snapshot".into())
}
fn verify_tooltip(
    before: &image::RgbaImage,
    after: &image::RgbaImage,
    target: &EditorTooltipProbe,
    scale: f32,
) -> Result<(), String> {
    if before.dimensions() != after.dimensions() {
        return Err("Window resized during hover".into());
    }
    let (mut left, mut top, mut right, mut bottom) = (after.width(), after.height(), 0, 0);
    for (x, y, pixel) in after.enumerate_pixels() {
        let in_button = (x as f32 - target.x * scale).abs() <= 18. * scale
            && (y as f32 - target.y * scale).abs() <= 18. * scale;
        if !in_button && pixel != before.get_pixel(x, y) {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }
    let wide = right.saturating_sub(left) as f32 / scale;
    let high = bottom.saturating_sub(top) as f32 / scale;
    // Measure rendered pixels beyond the hovered button: a partially occluded
    // inline hint must fail even when its declared rectangle has the right size.
    if wide + 1. < target.text_width + 14. || high + 1. < target.text_height + 8. {
        return Err(format!(
            "Rendered hint is {wide}x{high}; label needs {}x{} plus padding",
            target.text_width, target.text_height
        ));
    }
    Ok(())
}
fn key(ui: &WorkbenchWindow, key: slint::platform::Key) {
    use slint::platform::WindowEvent;
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
}
fn pointer(ui: &WorkbenchWindow, kind: u8, x: f32, y: f32) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(x, y);
    let button = PointerEventButton::Left;
    ui.window().dispatch_event(match kind {
        0 => WindowEvent::PointerPressed { position, button },
        1 => WindowEvent::PointerMoved { position },
        _ => WindowEvent::PointerReleased { position, button },
    });
}
fn click(ui: &WorkbenchWindow, x: f32, y: f32) {
    pointer(ui, 1, x, y);
    pointer(ui, 0, x, y);
    pointer(ui, 2, x, y);
}
fn snapshot(ui: &WorkbenchWindow, path: &Path) -> Result<(), String> {
    let p = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    image::save_buffer(
        path,
        p.as_bytes(),
        p.width(),
        p.height(),
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())
}
