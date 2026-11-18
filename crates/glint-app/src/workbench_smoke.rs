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

pub fn run(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
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
    let output = directory.to_owned();
    let done: Rc<RefCell<Result<(), String>>> = Rc::new(RefCell::new(Ok(())));
    let result = done.clone();
    let started = Instant::now();
    let mut phase = 0;
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(60),move||{
        let check=(|| -> Result<bool,String>{
            if started.elapsed()>Duration::from_secs(30){return Err(format!("Workbench QA timeout in phase {phase}"));}
            let Some(session)=CURRENT.with_borrow(Clone::clone)else{return Ok(false);};
            let ui=session.borrow().ui.clone_strong();
            if ui.get_busy(){return Ok(false);}
            if ui.get_picture().size().width==0{return Ok(false);}
            if ui.get_render_ticket()!=ui.get_rendered_ticket(){return Ok(false);}
            match phase {
                0=>{
                    snapshot(&ui,&output.join("image-light.png"))?;
                    ui.invoke_action("tool:1".into());ui.invoke_pointer(0,100.,180.,1.);ui.invoke_pointer(1,720.,280.,1.);ui.invoke_pointer(2,720.,280.,1.);
                    assert_eq!(session.borrow().model.state.marks.len(),1);assert!(ui.get_dirty());
                    ui.invoke_action("undo".into());assert!(session.borrow().model.state.marks.is_empty());assert!(!ui.get_dirty());ui.invoke_action("redo".into());
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
                    std::fs::write(output.join("result.txt"),"PASS: native image/video adapters, crop/text/history, dirty/quit guards, modal keyboard isolation, Esc cancellation, GIF seek/playback, image/MP4/GIF/MOV/MKV/WebM export panels, unavailable format guard, output scaling separate from document, codec checks, software snapshots, nonactivating clickable thumbnail. This does not verify physical input, mixed DPI or long recordings.").map_err(|e|e.to_string())?;
                    return Ok(true);
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
fn key(ui: &WorkbenchWindow, key: slint::platform::Key) {
    use slint::platform::WindowEvent;
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
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
