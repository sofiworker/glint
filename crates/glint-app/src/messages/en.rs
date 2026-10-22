use super::Message;

pub fn text(id: Message) -> &'static str {
    match id {
        Message::RecordingPlainRegion => {
            "Select an unannotated live desktop region first (at least 32 × 64 pixels); local images cannot be recorded."
        }
        Message::ScrollTitle => "Scrolling capture",
        Message::ScrollHint => {
            "Scroll down manually, wait for content to settle, then use your capture shortcut or Capture next. Keep at least 1/4 screen overlap. Exclude fixed headers, sidebars and scrollbars. No automatic scrolling."
        }
        Message::ScrollCapture => "Capture next",
        Message::ScrollUndo => "Undo fragment",
        Message::ScrollStop => "Stop & inspect",
        Message::ScrollEdit => "Confirm & annotate",
        Message::ScrollOverlap => "Overlap (px)",
        Message::ScrollAccept => "Accept join",
        Message::ScrollDiscard => "Discard pending",
        Message::ScrollPreview => "Preview seam",
        Message::ScrollResume => "Continue capture",
        Message::ScrollBusy => "Capturing / stitching…",
        Message::ScrollReady => "Fragments retained in memory",
        Message::ScrollDuplicate => "Identical fragment skipped. Scroll farther down and retry.",
        Message::ScrollUncertain => {
            "Match uncertain. Pending fragment retained. Adjust overlap and preview the seam; accept it, discard it or finish existing fragments."
        }
        Message::ScrollFailed => {
            "Capture / stitch failed. Existing fragments retained. Retry or finish the existing result."
        }
        Message::ScrollBudget => {
            "64 MiB source/result or 128 fragment limit reached. Undo a fragment or finish the current result."
        }
        Message::ScrollPlainRegion => {
            "Start scrolling capture from an unannotated desktop region inside one display (at least 32 × 64 pixels)."
        }
        Message::ScrollReviewHint => {
            "Scroll to inspect the joins (colored lines are preview only). Mouse wheel scrolls the long image in the annotation overlay; copy/save uses source pixels."
        }
        Message::CaptureFailureLabel => "Capture failed",
        Message::CaptureFocusUnavailable => "Click the capture overlay to focus it; Esc cancels.",
        Message::ExclusionInvalid => {
            "Invalid exclusion list. Use full .exe names, no paths/wildcards (up to 128). Correct or clear the list, save, then retry."
        }
        Message::ExclusionBlocked => {
            "Excluded app window detected (covered windows also count). Minimize/close it, or edit and save the exclusion list, then retry."
        }
        Message::ExclusionUnavailable => {
            "Exclusion check failed; capture blocked. Retry, or edit and save the exclusion list. These checks do not provide continuous protection."
        }
        Message::CompletionCopied => "Copied to clipboard.",
        Message::CompletionNotification => "Operation completed. Images stay local.",
        Message::CompletionSaved => "Image saved.",
        Message::CompletionPinned => "Desktop pin created.",
        Message::CompletionColor => "Color copied.",
        Message::CompletionApplied => "Pin changes applied.",
        Message::CompletionFallback => {
            "Windows notification submission failed; showing brief feedback instead."
        }
        Message::SessionConfigRecovery => {
            "Configuration needs recovery. Saved pin assets are preserved without reading or changing them. Confirm repaired settings before restoring or cleaning automatically."
        }
        Message::SessionFinishFirst => {
            "Finish or cancel the current screenshot/pin operation, then retry exit. Current editing is retained."
        }
        Message::SessionHelp => {
            "Off by default, separate from history. A normal tray exit saves source pixels (including covered content), current objects and window state, up to 256 MiB; old undo history is omitted. Restored pins are editable, visible and not click-through. Disable and Save deletes the saved session. A crash or forced exit keeps only the last successful session."
        }
        Message::SessionClear => "Delete saved pin session…",
        Message::SessionRestore => "Retry session restore",
        Message::SessionRetryExit => "Retry exit",
        Message::SessionExitWithout => "Exit without saving pins",
        Message::SessionCancel => "Cancel",
        Message::SessionBusy => "Processing pin session. Please wait…",
        Message::SessionIoFailed => {
            "Cannot access the pin session. Current pins are retained. Check directory permissions and retry."
        }
        Message::SessionRestoreFailed => {
            "Cannot restore: invalid/unsupported data, pin budget exceeded or window creation failed. The saved session is preserved; retry or delete it. Exit will not overwrite the session that could not be restored."
        }
        Message::SessionSaveFailed => {
            "Session save failed. Current pins and the last successful session are retained. Retry, or explicitly exit without saving the current pins."
        }
        Message::SessionClearFailed => {
            "Cannot delete the saved session. Image assets may remain in the configuration directory. Check permissions and retry cleanup."
        }
        Message::SessionEmpty => "No saved pin session.",
        Message::SessionRestored => {
            "Pin session restored. Pins are visible and not click-through; old undo history was not restored."
        }
        Message::SessionCleared => {
            "Saved session and session temporary files deleted. Current pins remain open."
        }
        Message::SessionClearHint => {
            "Delete only the saved pin session and unfinished session files in the configuration directory. Current pins and exported images stay. If restoration remains enabled, the next normal exit saves the current pins again."
        }
        Message::SessionExitHint => {
            "Exit without saving the current pins. The last successful session is preserved; new pins and unsaved changes will be lost."
        }
        Message::Workbench => "Auxiliary workbench",
        Message::WorkbenchCapture => "Capture region",
        Message::WorkbenchPrivacy => "History & privacy",
        Message::WorkbenchOutput => "Output settings",
        Message::WorkbenchHint => {
            "Open a local image in the annotation overlay. Screenshots work directly from the tray or shortcuts."
        }
        Message::WorkbenchNote => {
            "PNG/JPEG only. Files up to 64 MiB; decoded RGBA up to 64 MiB; canvas up to 128 MiB. The source file is never overwritten by opening it. Copy, save as or pin from the overlay. History browsing is planned and unavailable in this build."
        }
        Message::LocalOpen => "Open local image…",
        Message::LocalTitle => "Local image · Enter uses saved completion action · Esc cancels",
        Message::LocalHint => {
            "Local image · Copy, save as or create a new pin. The original file is preserved. Esc returns to the desktop."
        }
        Message::LocalFailed => {
            "Cannot open image. Choose a readable PNG/JPEG within the image budget and retry."
        }
        Message::TransferCancel => "Cancel preview",
        Message::TransferImport => "Import configuration",
        Message::TransferExport => "Export saved configuration",
        Message::TransferImportHint => {
            "Choose a TOML file up to 256 KiB. Review changes, then merge into the current draft. Save settings to register shortcuts and apply login startup; failures keep working settings. Invalid/unknown fields keep current values; conflicting shortcut imports keep the entire current binding group. The local onboarding marker and system permissions are not imported."
        }
        Message::TransferExportHint => {
            "Preview of saved settings and shortcuts. Unsaved changes, the onboarding marker, images, history assets and system permissions are excluded. Paths and privacy preferences are configuration values and are included. Choose a new file; existing files are never overwritten."
        }
        Message::TransferSelect => "Choose file…",
        Message::TransferApply => "Merge into draft",
        Message::TransferSkipped => "Skipped fields (current values preserved)",
        Message::TransferNoChanges => "No settings would change.",
        Message::TransferReadFailed => {
            "Cannot read the configuration file. The current draft is preserved; choose another file."
        }
        Message::TransferInvalid => {
            "Invalid UTF-8/TOML or missing/invalid version. The current draft is preserved."
        }
        Message::TransferVersion => {
            "This configuration version is not supported. The current draft is preserved."
        }
        Message::TransferTooLarge => {
            "The configuration exceeds 256 KiB. The current draft is preserved."
        }
        Message::TransferExportFailed => {
            "Export failed. Existing files and app configuration are preserved. Choose a new file and retry."
        }
        Message::TransferExportDone => "Configuration exported. App settings are unchanged.",
        Message::TransferDraftReady => {
            "Imported changes are in the draft. Review them and save settings to apply; Cancel restores saved settings."
        }
        Message::TransferStale => {
            "The draft changed after this preview. Choose the file again to review an updated preview."
        }
        Message::TransferBusy => {
            "Configuration task in progress. Please wait; preview cancellation does not undo a file write already started."
        }
        Message::ColorPickerEntry => "Pick a color",
        Message::ColorPickerHint => {
            "Frozen desktop: click or Enter copies HEX and exits on success; Esc cancels."
        }
        Message::AllScreensCapture => "Capture all screens",
        Message::CurrentScreenCapture => "Capture current screen",
        Message::AllScreensSelected => "All screens selected; adjust or annotate here.",
        Message::CurrentScreenSelected => "Pointer screen selected; adjust or annotate here.",
        Message::ScreenSelectionUnavailable => {
            "Cannot determine the screen from this snapshot; drag to select manually."
        }
        Message::DelayTitle => "Capture countdown",
        Message::DelayHint => {
            "The desktop stays live until capture. Move the pointer to the target window if needed."
        }
        Message::DelayCancel => "Cancel capture (Esc)",
        Message::RepeatOff => "Region memory is off. Select a region manually.",
        Message::RepeatEmpty => "No successful capture in this session. Select a region manually.",
        Message::RepeatChanged => "Display layout or DPI changed. Select a new region.",
        Message::RepeatReady => {
            "Previous region restored on a fresh capture, without old annotations."
        }
        Message::ToolbarCustomize => "Customize",
        Message::ToolbarTitle => "Customize capture toolbar",
        Message::ToolbarMore => "More tools",
        Message::ToolbarBoth => "Icons + labels",
        Message::ToolbarIcons => "Icons (hover for names)",
        Message::ToolbarFollow => "Follow selection / avoid edges",
        Message::ToolbarTop => "Fixed top",
        Message::ToolbarBottom => "Fixed bottom",
        Message::ToolbarGroups => "Show group separators",
        Message::ToolbarOrderHint => {
            "Drag ≡ or use arrows to reorder. Unchecked tools stay in More."
        }
        Message::ToolbarPreviewHint => {
            "Preview updates behind this dialog. Selection and annotations are retained."
        }
        Message::ToolbarApply => "Apply",
        Message::ToolbarReset => "Reset",
        Message::ToolbarPreview => "Live preview",
        Message::ToolbarMode => "Display mode",
        Message::ToolbarPosition => "Position",
        Message::ToolbarLayout => "Layout",
        Message::ToolbarGrid => "Grid",
        Message::ToolbarScroll => "Scroll horizontally",
        Message::ToolbarSingleRow => "Single row (all tools)",
        Message::ToolbarFixedWidth => "Fixed row width",
        Message::ToolbarColumns => "Columns",
        Message::ToolbarRows => "Rows",
        Message::ToolbarWidth => "Max width (px)",
        Message::ToolbarWidthHint => {
            "Enter the maximum width. The toolbar rounds down to fit whole buttons, so icons are never cut in half. Scroll horizontally to see the remaining tools."
        }
        Message::ToolbarSettingsPreviewHint => {
            "Appearance preview only. Save settings or apply customization to use this toolbar. Selection and annotations are retained."
        }
        Message::CapturePicker => "Pick color",
        Message::CaptureText => "Text",
        Message::CaptureTextHint => "Click to enter text · Enter places it; Esc cancels text",
        Message::CaptureTextPlace => "Place text",
        Message::CaptureTextCancel => "Cancel text",
        Message::CaptureTextSize => "Size",
        Message::CaptureTextFont => "Font",
        Message::CaptureTextLimit => "Single line, up to 80 characters",
        Message::CaptureTextRendering => "Rendering text…",
        Message::CaptureTextFailed => "Text failed; input preserved. Retry",
        Message::CapturePixelate => "Pixelate",
        Message::CapturePixelateHint => {
            "16 px blocks · Visual treatment; confidentiality is not guaranteed"
        }
        Message::CaptureMagnifier => "Frozen source pixels",
        Message::CapturePickerHint => "Original pixels · Click copies HEX",
        Message::CaptureCopyHex => "Copy HEX",
        Message::CaptureCopyRgb => "Copy RGB",
        Message::CaptureColorCopying => "Copying color…",
        Message::CaptureColorCopied => "Color copied",
        Message::CaptureColorCopyFailed => "Color copy failed; edits preserved. Retry",
        Message::CaptureColorOutside => {
            "Move inside the selection to pick an original pixel (without annotations)."
        }
        Message::CaptureTitle => "Glint · Capture",
        Message::CaptureDiagnosticTitle => "Glint · M0 capture probe",
        Message::CaptureSelect => "Select",
        Message::CapturePen => "Pen",
        Message::CaptureRedact => "Redact",
        Message::CaptureRectangle => "Rectangle",
        Message::CaptureArrow => "Arrow",
        Message::CaptureHighlight => "Highlight",
        Message::CaptureRectangleHint => "Drag to draw a rectangle",
        Message::CaptureArrowHint => "Drag from the arrow tail to its tip",
        Message::CaptureHighlightHint => {
            "Drag to highlight · Translucent; does not conceal content"
        }
        Message::CaptureUndo => "Undo",
        Message::CaptureRedo => "Redo",
        Message::CaptureSave => "Save",
        Message::CaptureFinish => "Finish",
        Message::CaptureCopy => "Copy",
        Message::CaptureCancel => "Cancel",
        Message::CaptureStrokeWidth => "Width",
        Message::CapturePenHint => "Drag to draw · Rounded strokes",
        Message::CaptureObjectHint => {
            "Drag to move; use handles for shape size / arrow endpoints; Delete removes it."
        }
        Message::CaptureObjectTextHint => {
            "Drag text to move; Edit text changes font size; Delete removes it."
        }
        Message::CaptureObjectDelete => "Delete object",
        Message::CaptureArrowFilled => "Filled arrow",
        Message::CaptureArrowOpen => "Open arrow",
        Message::CaptureTextEdit => "Edit text",
        Message::CaptureSelectHint => {
            "Click an annotation to select and drag it; blank space moves the selection, edge handles resize it. Double-click uses the saved completion action."
        }
        Message::CaptureRedactHint => {
            "Drag to cover sensitive content. Export flattens the solid redaction into the image."
        }
        Message::CaptureDragHint => "Drag to select a region · Esc cancels",
        Message::CaptureWindowHint => "Click to select this window, or drag a region · Esc cancels",
        Message::CaptureEditHint => "Annotate here · Enter copies · Esc cancels",
        Message::CaptureEditHintSave => "Annotate here · Enter saves · Esc cancels",
        Message::CapturePinUnavailable => {
            "Copy and pin is not connected yet. Edits retained; use Copy or Save."
        }
        Message::CaptureWindowUnavailable => {
            "Window detection unavailable; drag to select a region"
        }
        Message::DiagnosticsRenderer => "UI renderer: Slint · winit · software (CPU)",
        Message::DiagnosticsBackend => {
            "Capture backend: GDI (independent of UI rendering); WGC / DXGI are not implemented"
        }
        Message::DiagnosticsSubscribed => "Lock / sleep notifications: subscribed",
        Message::DiagnosticsUnavailable => "Lock / sleep notifications: unavailable",
        Message::DiagnosticsDisplays => "Displays",
        Message::DiagnosticsRemote => "Remote session",
        Message::DiagnosticsBounds => "Bounds",
        Message::DiagnosticsWorkArea => "Work area",
        Message::DiagnosticsPrimary => " · primary",
        Message::DiagnosticsDisplayFailed => "Display query failed",
        Message::DiagnosticsWorkingSet => "Working set",
        Message::DiagnosticsPrivate => "Private commit",
        Message::DiagnosticsHandles => "Handles",
        Message::DiagnosticsCpu => "Cumulative process CPU time (not utilization)",
        Message::DiagnosticsResourceFailed => "Resource query failed",
        Message::FontTitle => "Choose font",
        Message::FontSearch => "Search fonts",
        Message::FontCancel => "Cancel",
        Message::FontRetry => "Retry",
        Message::FontFailed => "Cannot read system fonts. Retry or cancel and type a name",
        Message::FontLoading => "Reading system fonts...",
        Message::FontEmpty => "No matching fonts. Change the search or cancel and type a name.",
        Message::FontHint => {
            "Choose to update only the current draft. Glyph coverage depends on the system."
        }
        Message::OutputFolderChoose => "Choose folder",
        Message::OutputFolderFailed => {
            "Cannot choose a folder. The original value is retained; retry or type a path."
        }
        Message::OutputFilenamePreview => {
            "Filename preview (example: 1280×720 px, 2026-10-18 14:32:05)"
        }
        Message::OutputFilenameInvalid => "Invalid filename template; edit the template.",
        Message::OutputReplaceTitle => "Confirm replacement",
        Message::OutputReplaceQuestion => "Replace this existing file?",
        Message::OutputReplaceHint => "Cancel keeps the current image edits.",
        Message::DiagnosticsTitle => "Runtime diagnostics",
        Message::DiagnosticsHint => {
            "Refresh reads displays and process counters without screenshots, window titles or clipboard content. Nothing is saved or uploaded. A snapshot does not establish compatibility or peak/idle resource use."
        }
        Message::DiagnosticsRefresh => "Refresh diagnostics",
        Message::DiagnosticsEmpty => {
            "Not queried. Refresh to read current displays and process resources."
        }
        Message::DiagnosticsLoading => {
            "Reading diagnostics; the previous snapshot remains visible."
        }
        Message::DiagnosticsFailed => "Refresh failed. Retry; any previous snapshot is retained",
        Message::LifecycleUnavailable => {
            "Lock/sleep notifications unavailable; automatic capture release is not active"
        }
        Message::LifecycleReleased => {
            "Lock, disconnect or sleep ended the capture. Start a new capture when ready; an export already in progress may still finish"
        }
        Message::CapturePreviewFailed => "Preview failed; output paused. Undo to recover",
        Message::CapturePreviewBudget => {
            "Annotation preview budget reached; edits retained, output paused. Undo or reselect to recover"
        }
        Message::CaptureMarkRejected => {
            "Annotation empty or object/stroke budget reached. Existing edits are preserved."
        }
        Message::CapturePointBudget => {
            "Stroke point limit reached; this stroke is complete. You can draw another stroke."
        }
        Message::CaptureProbePath => {
            "M0 saving requires an output.png argument. Current edits are preserved."
        }
        Message::CaptureOutputPending => {
            "The previous output is still finishing. Edits are preserved; try again shortly."
        }
        Message::CaptureGesturePending => {
            "Release the mouse to finish the current operation before exporting."
        }
        Message::CaptureImageFormats => "Use a .png, .jpg or .jpeg extension; edits are preserved.",
        Message::CaptureDialogFailed => "Cannot open the save dialog. Edits are preserved",
        Message::CaptureOutputBusy => {
            "Compositing and exporting… Edits will be preserved if output fails."
        }
        Message::CaptureOutputFailed => "Output failed. Edits are preserved",
        Message::CaptureSaveCancelled => {
            "Save cancelled. Selection and annotations are preserved; continue editing or retry."
        }
        Message::CaptureFilesSaved => "Files already saved (unchanged retry reuses them)",
        Message::CaptureSaveElsewhere => "Save As (skip automatic copy this time)",
        Message::CapturePin => "Pin",
        Message::PinPreparing => "Preparing desktop pin…",
        Message::PinPass => "Enable click-through…",
        Message::PinPassOff => "Turn off click-through",
        Message::PinPassTitle => "Remember how to rescue pins",
        Message::PinPassBody => {
            "Clicks will reach windows behind this pin. Use the rescue shortcut below, or tray → Rescue desktop pins, to show pins and turn off click-through."
        }
        Message::PinPassEnable => "Enable",
        Message::PinPassCancel => "Cancel",
        Message::PinPassActive => {
            "Click-through enabled. Use the rescue shortcut or tray to recover."
        }
        Message::PinRecoveryUnavailable => {
            "Rescue shortcut unavailable. Click-through stays off. Fix the shortcut and retry; tray rescue remains available."
        }
        Message::PinRescued => "Pin recovered. Click-through is off.",
        Message::PinActionFailed => "Cannot update pin; the image remains",
        Message::PinCopy => "Copy image",
        Message::PinSave => "Save image…",
        Message::PinRetry => "Retry / save elsewhere",
        Message::PinDismiss => "Close details",
        Message::PinOutputFailed => {
            "Output failed. The pin remains; retry copying or save elsewhere."
        }
        Message::PinCopied => "Image copied at original resolution.",
        Message::PinSaved => "Image saved. Click here for the path.",
        Message::PinOutputCancelled => "Save cancelled. The pin remains open.",
        Message::PinClipboardReading => "Reading clipboard image…",
        Message::PinClipboardCancelled => {
            "Clipboard pin cancelled after lock, disconnect or sleep. Trigger it again to retry."
        }
        Message::PinClipboardCreated => "Clipboard image pinned to desktop.",
        Message::PinClipboardFailed => {
            "Cannot pin clipboard image. Existing pins remain. Copy an image, then retry from the tray or shortcut"
        }
        Message::PinHint => "Drag the title bar; use the tray to hide or restore pins.",
        Message::PinFailed => {
            "Pin failed; edits remain. Previous copy/file steps may already have completed"
        }
        Message::PinBudget => {
            "Pin count or memory reservation limit reached. Close a pin or increase the budget."
        }
        Message::PinLabel => "Pin",
        Message::PinLocked => "Locked",
        Message::PinClose => "Close pin",
        Message::PinTop => "Toggle always on top",
        Message::PinUnlock => "Unlock position",
        Message::PinLock => "Lock position",
        Message::PinHide => "Hide pin (tray restores it)",
        Message::PinPlanned => {
            "Copy/save uses source pixels in the current rotation, without window zoom, opacity or controls. Edit reopens the source and annotation objects."
        }
        Message::PinRotate => "Rotate 90° clockwise",
        Message::PinEdit => "Continue annotation in place",
        Message::PinEditApply => "Apply",
        Message::PinEditTitle => {
            "Re-annotate pin · Enter uses the saved completion action · Esc cancels"
        }
        Message::PinEditSelectHint => {
            "Select and drag an annotation; use its handles to resize. Image bounds stay fixed. Double-click uses the saved completion action."
        }
        Message::PinEditHint => {
            "Edit in source orientation. Apply replaces this pin and keeps its rotation and appearance. Copy/save outputs this canvas only; cancel keeps the original."
        }
        Message::PinRotating => "Rotating; the current pin stays until ready…",
        Message::PinRotated => "Rotated. Copy/save uses this orientation at source resolution.",
        Message::SystemThemeUnavailable => {
            "Cannot read the system theme. Using light; choose a theme manually."
        }
        Message::UnsavedChanges => "Unsaved changes",
        Message::RecoveredBackup => {
            "Damaged configuration preserved. The last valid backup is loaded; Save repairs the file."
        }
        Message::RecoveredDefaults => {
            "Damaged configuration preserved. Defaults are loaded; Save repairs the file."
        }
        Message::UnsupportedConfig => {
            "Unsupported configuration version. The original is protected; defaults are read-only."
        }
        Message::UnreadableConfig => {
            "Cannot read or preserve the configuration. Check file access and reopen. Read-only mode."
        }
        Message::Settings => "Settings",
        Message::SettingsSubtitle => {
            "Open when needed; capture and annotation stay in the overlay."
        }
        Message::CloseToTray => "Close · Back to tray",
        Message::SaveSettings => "Save settings",
        Message::CancelChanges => "Cancel changes",
        Message::RestoreDefaults => "Restore defaults",
        Message::Unbound => "Not bound",
        Message::ContinueEditing => "Continue editing",
        Message::DiscardAndLeave => "Discard and leave",
        Message::SaveAndLeave => "Save and leave",
        Message::ClearBinding => "Clear binding",
        Message::AcceptShortcut => "Use this shortcut",
        Message::DevelopmentStatus => "Capture · Annotate · Pin",
        Message::DraftExplanation => "Click Save settings to apply your changes.",
        Message::SystemPreference => "System preference:",
        Message::ManualLanguage => "Manual choice takes priority",
        Message::LanguageFallback => "Unsupported language; using English",
        Message::AutomaticLanguage => "Automatic match",
        Message::UnsavedTitle => "Unsaved settings",
        Message::ResetTitle => "Restore default settings?",
        Message::RecordShortcutTitle => "Press a new shortcut",
        Message::UnsavedBody => {
            "Save before leaving, discard changes, or continue editing. A failed save keeps the draft."
        }
        Message::ResetBody => {
            "Reset the settings draft only. Saved preferences change only after Save succeeds."
        }
        Message::ShortcutRegistrationFailed => {
            "Global shortcut registration failed. Use the tray to capture or rebind shortcuts."
        }
        Message::OptionUnavailable => "This option cannot be changed yet.",
        Message::RecordShortcutBody => {
            "Press a chord or function key. Esc cancels; Tab moves to the buttons. Confirm, then save settings."
        }
        Message::ShortcutInvalid => {
            "Invalid shortcut or duplicate action binding. Try another combination."
        }
        Message::ShortcutCandidate => "Enter confirms; Esc cancels. Save settings to apply.",
        Message::ShortcutServiceStopped => {
            "The shortcut service stopped. Use the tray and restart Glint."
        }
        Message::SettingsSaved => "Settings saved.",
        Message::SaveRollbackFailed => {
            "Save and system-settings rollback failed. The draft is retained; check login startup and shortcuts, then retry."
        }
        Message::SaveFailed => {
            "Save failed; the draft is retained. Check shortcut conflicts, login startup or configuration-file access, then retry."
        }
    }
}
